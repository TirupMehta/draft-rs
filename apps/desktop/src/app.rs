//! The one Draft window: live state, the dictation loop, and the engine
//! hand-off. The model lives on its own thread; this view only polls events.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use draft_audio::Recorder;
use draft_core::settings::Settings;
use draft_engine::{ComputeBackend, EngineTimings};
use draft_hotkey::{HotkeyConfig, HotkeyEvent, HotkeyListener};
use gpui::{
    AnyElement, AnyWindowHandle, Context, Div, Entity, IntoElement, Render, Rgba, Window, div,
    prelude::*, px,
};
use tray_icon::menu::MenuEvent;
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconEvent};

use crate::hud::{DraftHud, HudSnapshot};
use crate::theme;
use crate::tray;
use crate::window_control;
use crate::worker::{self, Job};

const HOTKEYS: [&str; 6] = [
    "right-ctrl",
    "right-alt",
    "f8",
    "f9",
    "pause",
    "scroll-lock",
];
const LEVEL_TEST_LENGTH: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Loading,
    Ready,
    Listening,
    Transcribing,
    Error,
}

struct LastResult {
    text: String,
    engine_ms: u128,
    audio_s: f64,
    _timings: EngineTimings,
    paste_error: Option<String>,
}

struct LevelTest {
    recorder: Recorder,
    started: Instant,
}

pub struct DraftApp {
    phase: Phase,
    engine_ready: bool,
    backend: Option<String>,
    device: Option<String>,
    load_ms: Option<u128>,
    _hotkey: Option<HotkeyListener>,
    hotkey: HotkeyConfig,
    hotkey_events: Option<Receiver<HotkeyEvent>>,
    hotkey_error: Option<String>,
    hotkey_label: String,
    recorder: Option<Recorder>,
    level_test: Option<LevelTest>,
    microphone: Option<String>,
    microphones: Vec<String>,
    default_microphone: Option<String>,
    jobs: Sender<Job>,
    worker_events: Receiver<worker::Event>,
    pending_jobs: usize,
    partial: Option<String>,
    preview_pending: bool,
    preview_at: Instant,
    serial: u64,
    last: Option<LastResult>,
    error: Option<String>,
    notice: Option<String>,
    model: PathBuf,
    settings: Settings,
    settings_path: PathBuf,
    history: Option<draft_core::history::History>,
    hud: Entity<DraftHud>,
    hud_snapshot: Option<HudSnapshot>,
    hud_visible: bool,
    hud_hwnd: Option<isize>,
    main_window: Option<AnyWindowHandle>,
    main_hwnd: Option<isize>,
    tray: Option<TrayIcon>,
    clock: Instant,
}

impl DraftApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        _cx: &mut Context<Self>,
        model: PathBuf,
        backend: ComputeBackend,
        paste: bool,
        hotkey: HotkeyConfig,
        microphone: Option<String>,
        settings: Settings,
        settings_path: PathBuf,
        hud: Entity<DraftHud>,
    ) -> Self {
        let microphones = match draft_audio::input_devices() {
            Ok(devices) => devices,
            Err(error) => {
                tracing::warn!(%error, "could not list microphones");
                Vec::new()
            }
        };
        let default_microphone = draft_audio::default_input_device_name();
        let mut notice = None;
        if let Some(saved) = microphone.as_ref() {
            if !microphones.is_empty() && !microphones.iter().any(|device| device == saved) {
                notice = Some(format!(
                    "Saved microphone \"{saved}\" is unavailable; the system default will be used."
                ));
            }
        }

        let model_display = model.clone();
        let (jobs, worker_events) = worker::spawn(model, backend, paste);
        let hotkey_label = hotkey.name.to_owned();
        let tray = match tray::build(&hotkey_label) {
            Ok(tray) => Some(tray),
            Err(error) => {
                tracing::warn!(%error, "tray icon unavailable");
                None
            }
        };
        let (listener, hotkey_events, hotkey_error) = match draft_hotkey::start(hotkey) {
            Ok((listener, events)) => (Some(listener), Some(events), None),
            Err(error) => (
                None,
                None,
                Some(format!("The global hotkey is unavailable: {error:#}")),
            ),
        };
        let initial_error = hotkey_error.clone();
        let history = match draft_core::history::History::open(
            draft_core::paths::history_path(),
            draft_core::history::HistoryPolicy::default(),
            draft_core::history::now_ms(),
        ) {
            Ok(history) => {
                if history.recovered_from_corruption {
                    tracing::warn!(
                        path = %history.path().display(),
                        "the previous history file was malformed and was moved aside"
                    );
                }
                Some(history)
            }
            Err(error) => {
                tracing::warn!(%error, "history is unavailable; dictation will continue");
                None
            }
        };

        Self {
            phase: if hotkey_error.is_some() {
                Phase::Error
            } else {
                Phase::Loading
            },
            engine_ready: false,
            backend: None,
            device: None,
            load_ms: None,
            _hotkey: listener,
            hotkey,
            hotkey_events,
            hotkey_error,
            hotkey_label,
            recorder: None,
            level_test: None,
            microphone,
            microphones,
            default_microphone,
            jobs,
            worker_events,
            pending_jobs: 0,
            partial: None,
            preview_pending: false,
            preview_at: Instant::now(),
            serial: 0,
            last: None,
            error: initial_error,
            notice,
            model: model_display,
            settings,
            settings_path,
            history,
            hud,
            hud_snapshot: None,
            hud_visible: false,
            hud_hwnd: None,
            main_window: None,
            main_hwnd: None,
            tray,
            clock: Instant::now(),
        }
    }

    pub fn has_tray(&self) -> bool {
        self.tray.is_some()
    }

    pub fn attach_windows(
        &mut self,
        main_window: Option<AnyWindowHandle>,
        main_hwnd: Option<isize>,
        hud_hwnd: Option<isize>,
    ) {
        self.main_window = main_window;
        self.main_hwnd = main_hwnd;
        self.hud_hwnd = hud_hwnd;
        if let Some(hwnd) = self.hud_hwnd {
            window_control::set_hud_visible(hwnd, false);
        }
    }

    pub fn poll(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        while let Ok(event) = self.worker_events.try_recv() {
            self.handle_worker(event);
            changed = true;
        }
        if let Some(events) = self.hotkey_events.take() {
            while let Ok(event) = events.try_recv() {
                self.handle_hotkey(event);
                changed = true;
            }
            self.hotkey_events = Some(events);
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
                | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    self.show_main(cx);
                    changed = true;
                }
                _ => {}
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.as_ref() {
                tray::SHOW_ID => {
                    self.show_main(cx);
                    changed = true;
                }
                tray::HIDE_ID => {
                    self.hide_main(cx);
                    changed = true;
                }
                tray::PASTE_LAST_ID => {
                    self.paste_last();
                    changed = true;
                }
                tray::QUIT_ID => {
                    cx.quit();
                    return;
                }
                _ => {}
            }
        }
        if self
            .level_test
            .as_ref()
            .is_some_and(|test| test.started.elapsed() >= LEVEL_TEST_LENGTH)
        {
            self.level_test = None;
            self.notice = Some("Microphone check finished.".to_owned());
            changed = true;
        }
        if self.recorder.is_some() || self.level_test.is_some() {
            // Keep the level meter moving while audio is open.
            changed = true;
        }
        self.maybe_preview();
        self.sync_hud(cx);
        if changed {
            cx.notify();
        }
    }

    fn show_main(&mut self, cx: &mut Context<Self>) {
        if let Some(hwnd) = self.main_hwnd {
            window_control::show(hwnd);
        }
        if let Some(handle) = self.main_window {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
    }

    fn hide_main(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.main_window {
            let _ = handle.update(cx, |_, window, _| {
                window_control::hide(window);
            });
        }
    }

    fn choose_hotkey(&mut self, config: HotkeyConfig, cx: &mut Context<Self>) {
        if self.hotkey == config {
            // The runtime key can differ from the saved key when a CLI flag
            // overrides this launch. Clicking the visible pill should still make
            // that choice durable for the next launch.
            if self.settings.hotkey != config.id() {
                self.settings.hotkey = config.id().to_owned();
                if self.save_settings() {
                    self.notice = Some(format!("Dictation key: {}", config.name));
                }
                cx.notify();
            }
            return;
        }
        if self.recorder.is_some() {
            self.notice = Some("Finish the current dictation before changing the key.".to_owned());
            cx.notify();
            return;
        }
        if self.restart_hotkey(config) {
            self.settings.hotkey = config.id().to_owned();
            if self.save_settings() {
                self.notice = Some(format!("Dictation key: {}", config.name));
            }
        }
        cx.notify();
    }

    /// Replace the global hook. The old listener is dropped first so two hooks
    /// never fight over the same physical key. If the new key cannot be
    /// registered the previous one is restored.
    fn restart_hotkey(&mut self, config: HotkeyConfig) -> bool {
        let previous = self.hotkey;
        self._hotkey = None;
        self.hotkey_events = None;

        match draft_hotkey::start(config) {
            Ok((listener, events)) => {
                self._hotkey = Some(listener);
                self.hotkey_events = Some(events);
                self.hotkey = config;
                self.hotkey_label = config.name.to_owned();
                self.update_tray_tooltip();
                self.hotkey_error = None;
                self.error = None;
                if self.engine_ready {
                    self.refresh_phase();
                } else {
                    self.phase = Phase::Loading;
                }
                true
            }
            Err(error) => {
                tracing::warn!(%error, key = config.name, "could not switch the dictation key");
                match draft_hotkey::start(previous) {
                    Ok((listener, events)) => {
                        self._hotkey = Some(listener);
                        self.hotkey_events = Some(events);
                        self.hotkey_error = None;
                        self.error = None;
                        self.notice = Some(format!(
                            "Could not use {}; kept {}.",
                            config.name, previous.name
                        ));
                        if self.engine_ready {
                            self.refresh_phase();
                        } else {
                            self.phase = Phase::Loading;
                        }
                    }
                    Err(restore_error) => {
                        let message =
                            format!("The global hotkey is unavailable: {restore_error:#}");
                        self.hotkey_error = Some(message.clone());
                        self.error = Some(message);
                        self.phase = Phase::Error;
                    }
                }
                false
            }
        }
    }

    fn update_tray_tooltip(&self) {
        let Some(tray) = self.tray.as_ref() else {
            return;
        };
        let tooltip = format!("Draft - hold {} to dictate", self.hotkey_label);
        if let Err(error) = tray.set_tooltip(Some(tooltip.as_str())) {
            tracing::warn!(%error, "could not update the tray tooltip");
        }
    }

    fn choose_microphone(&mut self, microphone: Option<String>, cx: &mut Context<Self>) {
        if self.recorder.is_some() {
            self.notice =
                Some("Finish the current dictation before changing the microphone.".to_owned());
            cx.notify();
            return;
        }
        let changed = self.microphone != microphone;
        self.level_test = None;
        self.microphone = microphone.clone();
        self.settings.microphone = microphone;
        if changed && self.save_settings() {
            self.notice = Some("Microphone updated.".to_owned());
        }
        cx.notify();
    }

    fn test_microphone(&mut self, cx: &mut Context<Self>) {
        if self.recorder.is_some() {
            self.notice =
                Some("Finish the current dictation before testing the microphone.".to_owned());
            cx.notify();
            return;
        }
        if self.level_test.is_some() {
            self.level_test = None;
            self.notice = Some("Microphone check stopped.".to_owned());
            cx.notify();
            return;
        }

        match Recorder::start_on(self.microphone.as_deref()) {
            Ok(recorder) => {
                self.notice = Some(if recorder.fell_back() {
                    format!(
                        "Saved microphone is unavailable; testing {}.",
                        microphone_display(recorder.device())
                    )
                } else {
                    "Speak to check the level. The check stops after four seconds.".to_owned()
                });
                self.level_test = Some(LevelTest {
                    recorder,
                    started: Instant::now(),
                });
            }
            Err(error) => {
                self.notice = Some(format!("Microphone: {error:#}"));
            }
        }
        cx.notify();
    }

    fn input_level(&self) -> f32 {
        self.recorder
            .as_ref()
            .or_else(|| self.level_test.as_ref().map(|test| &test.recorder))
            .map(Recorder::level)
            .unwrap_or(0.0)
    }

    fn save_settings(&mut self) -> bool {
        match self.settings.save(&self.settings_path) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, "could not save settings");
                self.notice = Some(format!("Could not save settings: {error:#}"));
                false
            }
        }
    }

    fn use_default_model(&mut self, cx: &mut Context<Self>) {
        if self.settings.model.is_none() {
            return;
        }
        self.settings.model = None;
        if self.save_settings() {
            self.notice = Some("The default model will be used next launch.".to_owned());
        }
        cx.notify();
    }

    fn paste_last(&mut self) {
        let Some(last) = self.last.as_ref() else {
            self.notice = Some("No completed dictation yet.".to_owned());
            return;
        };
        let text = last.text.clone();
        self.notice = Some("Pasting the last dictation.".to_owned());
        std::thread::spawn(move || {
            if let Err(error) = draft_paste::paste_text(&text) {
                tracing::warn!(%error, "could not paste the last dictation");
            }
        });
    }

    fn sync_hud(&mut self, cx: &mut Context<Self>) {
        let snapshot = match self.phase {
            Phase::Listening => Some(HudSnapshot::listening(
                &self.hotkey_label,
                self.partial.clone(),
                self.input_level(),
            )),
            Phase::Transcribing => Some(HudSnapshot::transcribing()),
            Phase::Loading | Phase::Ready | Phase::Error => None,
        };
        if snapshot == self.hud_snapshot {
            return;
        }

        let was_visible = self.hud_visible;
        let visible = snapshot.is_some();
        self.hud_snapshot = snapshot.clone();
        self.hud.update(cx, |hud, cx| hud.set(snapshot, cx));
        if visible != was_visible {
            if let Some(hwnd) = self.hud_hwnd {
                window_control::set_hud_visible(hwnd, visible);
            }
            self.hud_visible = visible;
        }
    }

    /// While the key is held, transcribe the last few seconds now and then so
    /// the HUD can show what was heard. The final pass still uses the complete
    /// clip, and previews never paste.
    fn maybe_preview(&mut self) {
        if self.phase != Phase::Listening || self.preview_pending {
            return;
        }
        if self.preview_at.elapsed() < Duration::from_millis(650) {
            return;
        }
        let Some(recorder) = self.recorder.as_ref() else {
            return;
        };
        let Ok(recording) = recorder.preview(12) else {
            return;
        };
        if recording.samples.len() < 12_000 {
            return;
        }
        let job = Job {
            samples: recording.samples,
            serial: self.serial,
            preview: true,
        };
        if self.jobs.send(job).is_ok() {
            self.preview_pending = true;
            self.preview_at = Instant::now();
        }
    }

    fn handle_worker(&mut self, event: worker::Event) {
        match event {
            worker::Event::Ready {
                backend,
                device,
                load_ms,
            } => {
                self.backend = Some(backend);
                self.device = Some(device);
                self.load_ms = Some(load_ms);
                self.engine_ready = true;
                if self.hotkey_error.is_none() {
                    self.error = None;
                }
                self.refresh_phase();
            }
            worker::Event::Failed(error) => {
                self.engine_ready = false;
                self.phase = Phase::Error;
                self.error = Some(error);
            }
            worker::Event::Partial { text, serial } => {
                if serial != self.serial {
                    return;
                }
                self.preview_pending = false;
                if self.phase == Phase::Listening && !text.trim().is_empty() {
                    self.partial = Some(text);
                }
            }
            worker::Event::Transcribed {
                text,
                elapsed,
                timings,
                audio_s,
                paste_error,
            } => {
                self.pending_jobs = self.pending_jobs.saturating_sub(1);
                if let Some(history) = self.history.as_mut() {
                    let duration_ms = (audio_s * 1000.0).round() as u64;
                    if let Err(error) =
                        history.add(&text, Some(duration_ms), draft_core::history::now_ms())
                    {
                        tracing::warn!(%error, "could not record the transcript in history");
                    }
                }
                self.last = Some(LastResult {
                    text,
                    engine_ms: elapsed.as_millis(),
                    audio_s,
                    _timings: timings,
                    paste_error,
                });
                self.partial = None;
                self.notice = None;
                self.refresh_phase();
            }
            worker::Event::Empty { elapsed } => {
                self.pending_jobs = self.pending_jobs.saturating_sub(1);
                self.partial = None;
                self.notice = Some(format!(
                    "No speech detected in that clip ({:.0} ms).",
                    elapsed.as_secs_f64() * 1000.0
                ));
                self.refresh_phase();
            }
            worker::Event::JobFailed { error } => {
                self.pending_jobs = self.pending_jobs.saturating_sub(1);
                self.partial = None;
                self.error = Some(format!("Transcription failed: {error}"));
                self.refresh_phase();
            }
        }
    }

    fn handle_hotkey(&mut self, event: HotkeyEvent) {
        match event {
            HotkeyEvent::Pressed => self.start_capture(),
            HotkeyEvent::Released => self.stop_capture(),
            HotkeyEvent::Cancel => self.cancel_capture(),
        }
    }

    fn start_capture(&mut self) {
        if !self.engine_ready {
            self.notice = Some("The model is still preparing.".to_owned());
            return;
        }
        if self.recorder.is_some() {
            return;
        }

        self.error = None;
        self.notice = None;
        self.partial = None;
        self.preview_pending = false;
        self.preview_at = Instant::now();
        self.serial = self.serial.wrapping_add(1);
        // A level check holds the microphone open. A real dictation takes over
        // that stream instead of waiting for the check to expire.
        self.level_test = None;
        match Recorder::start_on(self.microphone.as_deref()) {
            Ok(recorder) => {
                if recorder.fell_back() {
                    self.notice = Some(format!(
                        "Saved microphone is unavailable; using {}.",
                        microphone_display(recorder.device())
                    ));
                }
                self.recorder = Some(recorder);
                self.phase = Phase::Listening;
            }
            Err(error) => {
                self.error = Some(format!("Microphone: {error:#}"));
                self.phase = Phase::Error;
            }
        }
    }

    fn stop_capture(&mut self) {
        let Some(recorder) = self.recorder.take() else {
            return;
        };

        match recorder.stop() {
            Ok(recording) => {
                if recording.duration() < Duration::from_millis(300) {
                    self.notice = Some(format!(
                        "Too short. Hold {} while you speak.",
                        self.hotkey_label
                    ));
                    self.refresh_phase();
                    return;
                }

                let job = Job {
                    samples: recording.samples,
                    serial: self.serial,
                    preview: false,
                };
                if self.jobs.send(job).is_err() {
                    self.error = Some("The engine worker stopped.".to_owned());
                    self.phase = Phase::Error;
                } else {
                    self.pending_jobs += 1;
                    self.notice = None;
                    self.refresh_phase();
                }
            }
            Err(error) => {
                self.error = Some(format!("Microphone: {error:#}"));
                self.phase = Phase::Error;
            }
        }
    }

    fn cancel_capture(&mut self) {
        if let Some(recorder) = self.recorder.take() {
            let _ = recorder.stop();
            self.partial = None;
            self.notice = Some("Cancelled.".to_owned());
            self.refresh_phase();
        }
    }

    fn refresh_phase(&mut self) {
        if self.hotkey_error.is_some() {
            self.phase = Phase::Error;
            return;
        }
        if !self.engine_ready {
            return;
        }
        self.phase = if self.recorder.is_some() {
            Phase::Listening
        } else if self.pending_jobs > 0 {
            Phase::Transcribing
        } else {
            Phase::Ready
        };
    }

    fn status(&self) -> (&'static str, String, Rgba) {
        match self.phase {
            Phase::Loading => (
                "Loading model",
                "The local model is preparing. Dictation starts when it is ready.".to_owned(),
                theme::accent(),
            ),
            Phase::Ready => (
                "Ready",
                format!(
                    "Hold {} and speak. Release to paste at the cursor.",
                    self.hotkey_label
                ),
                theme::success(),
            ),
            Phase::Listening => (
                "Listening",
                format!(
                    "Release {} to transcribe. Escape cancels.",
                    self.hotkey_label
                ),
                theme::accent(),
            ),
            Phase::Transcribing => (
                "Transcribing",
                "The local model is turning your speech into text.".to_owned(),
                theme::accent(),
            ),
            Phase::Error => (
                "Needs attention",
                self.error
                    .clone()
                    .unwrap_or_else(|| "Something went wrong.".to_owned()),
                theme::danger(),
            ),
        }
    }
}

impl Render for DraftApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (status, detail, color) = self.status();
        let engine = engine_label(self.backend.as_deref(), self.device.as_deref());
        let model_name = model_label(&self.model);
        let model_detail = model_quantization(&self.model);
        let model_path = self.model.display().to_string();
        let microphone_label = match &self.microphone {
            Some(name) => microphone_display(name),
            None => match &self.default_microphone {
                Some(name) => format!("System default · {}", microphone_display(name)),
                None => "System default".to_owned(),
            },
        };
        let level = self.input_level();
        let level_fill = (level.sqrt() * 1.25).clamp(0.0, 1.0);
        let testing = self.level_test.is_some();
        let phase = self.clock.elapsed().as_secs_f32() * 7.0;

        let hotkey_pills = HOTKEYS
            .iter()
            .enumerate()
            .filter_map(|(index, name)| {
                let config = HotkeyConfig::named(name)?;
                Some(
                    choice_pill(config.name, self.hotkey == config)
                        .id(("hotkey", index))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.choose_hotkey(config, cx)),
                        ),
                )
            })
            .collect::<Vec<_>>();
        let mut microphone_pills = vec![
            choice_pill("System default", self.microphone.is_none())
                .id("microphone-default")
                .on_click(cx.listener(|this, _, _, cx| this.choose_microphone(None, cx))),
        ];
        for (index, name) in self.microphones.iter().enumerate() {
            let selected = self.microphone.as_deref() == Some(name.as_str());
            let is_default = self.default_microphone.as_deref() == Some(name.as_str());
            let display = microphone_display(name);
            let label = if is_default {
                format!("{display} · default")
            } else {
                display
            };
            let device = name.clone();
            microphone_pills.push(
                choice_pill(label, selected)
                    .id(("microphone", index))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose_microphone(Some(device.clone()), cx)
                    })),
            );
        }
        let model_action: AnyElement = if self.settings.model.is_some() {
            div()
                .id("model-default")
                .px_3()
                .py(px(6.0))
                .rounded_full()
                .border_1()
                .border_color(theme::border())
                .text_xs()
                .text_color(theme::text_muted())
                .cursor_pointer()
                .hover(|style| style.text_color(theme::text()))
                .on_click(cx.listener(|this, _, _, cx| this.use_default_model(cx)))
                .child("Use default")
                .into_any_element()
        } else {
            div()
                .px_3()
                .py(px(6.0))
                .rounded_full()
                .bg(theme::elevated())
                .text_xs()
                .text_color(theme::text_muted())
                .child("Default")
                .into_any_element()
        };

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_lg()
                            .bg(theme::accent())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme::background())
                                    .child("D"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme::text())
                                    .child("Draft"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .child("On-device dictation"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child("Closing hides to tray"),
                    )
                    .child(
                        div()
                            .id("close")
                            .px_3()
                            .py(px(6.0))
                            .rounded_full()
                            .border_1()
                            .border_color(theme::border())
                            .text_xs()
                            .text_color(theme::text_muted())
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme::text()))
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.tray.is_some() {
                                    window_control::hide(window);
                                } else {
                                    cx.quit();
                                }
                            }))
                            .child(if self.tray.is_some() { "Hide" } else { "Quit" }),
                    ),
            );

        let hero_detail = match self.phase {
            Phase::Listening => "Speak normally. Words appear here before you release.".to_owned(),
            Phase::Transcribing => "Turning speech into text on this device.".to_owned(),
            _ => detail.clone(),
        };

        let hero_body: AnyElement =
            match self.phase {
                Phase::Listening => {
                    let partial = self
                        .partial
                        .as_deref()
                        .map(preview)
                        .filter(|text| !text.trim().is_empty());
                    let (words, words_color) = match partial {
                        Some(text) => (text, theme::text()),
                        None => (
                            "Speak now — your words appear here as you talk.".to_owned(),
                            theme::text_muted(),
                        ),
                    };
                    div()
                        .bg(theme::elevated())
                        .rounded_lg()
                        .p_5()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(div().text_lg().text_color(words_color).child(words))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_4()
                                .child(level_bars(level, 24, 30.0, 3.0, phase, theme::accent()))
                                .child(div().text_xs().text_color(theme::text_muted()).child(
                                    format!(
                                        "Release {} to transcribe · Esc cancels",
                                        self.hotkey_label
                                    ),
                                )),
                        )
                        .into_any_element()
                }
                Phase::Transcribing => div()
                    .bg(theme::elevated())
                    .rounded_lg()
                    .p_5()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(level_bars(
                        0.0,
                        24,
                        24.0,
                        3.0,
                        phase * 0.6,
                        theme::text_muted(),
                    ))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme::text_muted())
                            .child("Finishing on this device…"),
                    )
                    .into_any_element(),
                _ => {
                    let (title, hint) = match self.phase {
                        Phase::Loading => (
                            "Warming the model",
                            "Dictation starts as soon as the local model is ready.",
                        ),
                        Phase::Error => (
                            "Check the message below",
                            "Fix the problem, then restart Draft.",
                        ),
                        _ => ("Hold to dictate", "Release to paste at your cursor."),
                    };
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(keycap(self.hotkey_label.clone()))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_sm().text_color(theme::text()).child(title))
                                .child(div().text_xs().text_color(theme::text_muted()).child(hint)),
                        )
                        .into_any_element()
                }
            };

        let hero = div()
            .bg(theme::surface())
            .border_1()
            .border_color(theme::border())
            .rounded_lg()
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().w(px(9.0)).h(px(9.0)).rounded_full().bg(color))
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme::text())
                                    .child(status),
                            ),
                    )
                    .child(
                        div()
                            .px_3()
                            .py(px(4.0))
                            .rounded_full()
                            .bg(theme::background())
                            .border_1()
                            .border_color(theme::border())
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child("LOCAL"),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme::text_muted())
                    .child(hero_detail),
            )
            .child(hero_body)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_6()
                    .pt_1()
                    .child(stat("Shortcut", self.hotkey_label.clone()))
                    .child(stat("Model", format!("{model_name} · {model_detail}")))
                    .child(stat("Engine", engine)),
            );

        let preferences = div()
            .bg(theme::surface())
            .border_1()
            .border_color(theme::border())
            .rounded_lg()
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme::text())
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child("Saved on this machine"),
                    ),
            )
            .child(divider())
            .child(section(
                "Dictation key",
                "Hold to dictate · release to paste",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().flex().flex_wrap().gap_2().children(hotkey_pills))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child("Changes apply immediately. Esc cancels an active capture."),
                    ),
            ))
            .child(divider())
            .child(section(
                "Microphone",
                "Input device",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme::text())
                                            .child(microphone_label),
                                    )
                                    .child(div().text_xs().text_color(theme::text_muted()).child(
                                        if self.microphone.is_some() {
                                            "Selected microphone"
                                        } else {
                                            "Follows the system default"
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(120.0))
                                            .h(px(5.0))
                                            .bg(theme::background())
                                            .rounded_full()
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(px(120.0 * level_fill))
                                                    .bg(theme::accent())
                                                    .rounded_full(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("microphone-test")
                                            .px_3()
                                            .py(px(6.0))
                                            .rounded_full()
                                            .border_1()
                                            .border_color(theme::border())
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .cursor_pointer()
                                            .hover(|style| style.text_color(theme::text()))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.test_microphone(cx)
                                            }))
                                            .child(if testing { "Stop test" } else { "Test" }),
                                    ),
                            ),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(microphone_pills))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child("The level moves while dictating or during a four-second test."),
                    ),
            ))
            .child(divider())
            .child(section(
                "Model",
                "Local GGUF · never downloaded at runtime",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme::text())
                                            .child(format!("{model_name} · {model_detail}")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .truncate()
                                            .child(model_path),
                                    ),
                            )
                            .child(model_action),
                    )
                    .child(div().text_xs().text_color(theme::text_muted()).child(
                        "Set a different GGUF with --model or settings.json, then restart.",
                    )),
            ));

        let mut content = div()
            .id("draft-content")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .gap_4()
            .overflow_y_scroll()
            .child(hero);

        if let Some(last) = &self.last {
            let mut card = div()
                .bg(theme::surface())
                .border_1()
                .border_color(theme::border())
                .rounded_lg()
                .p_5()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Last dictation"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(format!(
                                    "{:.0} ms engine · {:.1}s audio",
                                    last.engine_ms as f64, last.audio_s
                                )),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::text())
                        .child(preview(&last.text)),
                );
            if let Some(error) = &last.paste_error {
                card = card.child(
                    div()
                        .text_xs()
                        .text_color(theme::danger())
                        .child(format!("Paste warning: {error}")),
                );
            }
            content = content.child(card);
        }

        content = content.child(preferences);

        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .bg(theme::surface())
                    .border_1()
                    .border_color(theme::danger())
                    .rounded_lg()
                    .p_4()
                    .text_xs()
                    .text_color(theme::danger())
                    .child(error.clone()),
            );
        }
        if let Some(notice) = &self.notice {
            content = content.child(
                div()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child(notice.clone()),
            );
        }

        div()
            .id("draft-root")
            .size_full()
            .bg(theme::background())
            .flex()
            .flex_col()
            .p_6()
            .gap_4()
            .child(header)
            .child(content)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child("Local audio. Local text. Nothing leaves this machine."),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                    ),
            )
    }
}

fn microphone_display(name: &str) -> String {
    // Windows friendly names occasionally carry a space before the closing
    // parenthesis, e.g. "Headset (Rockerz 425 )". Keep the stored name exact for
    // device matching and clean only what the person sees.
    name.replace(" )", ")")
}

fn engine_label(backend: Option<&str>, device: Option<&str>) -> String {
    let Some(backend) = backend else {
        return "Preparing".to_owned();
    };
    let family = backend.trim_end_matches(|ch: char| ch.is_ascii_digit());
    if family.is_empty() {
        return device.unwrap_or(backend).to_owned();
    }
    if family.eq_ignore_ascii_case("cpu") {
        return "CPU".to_owned();
    }
    match device {
        Some(device) if device.starts_with(family) => {
            let index = device[family.len()..].trim();
            if index.is_empty() {
                format!("{family} · GPU")
            } else {
                format!("{family} · GPU {index}")
            }
        }
        Some(device) => format!("{family} · {device}"),
        None => family.to_owned(),
    }
}

fn choice_pill(label: impl IntoElement, selected: bool) -> Div {
    let pill = div()
        .px_3()
        .py(px(6.0))
        .rounded_full()
        .border_1()
        .text_xs()
        .cursor_pointer();
    if selected {
        pill.bg(theme::elevated())
            .border_color(theme::accent())
            .text_color(theme::text())
            .child(label)
    } else {
        pill.bg(theme::background())
            .border_color(theme::border())
            .text_color(theme::text_muted())
            .hover(|style| {
                style
                    .text_color(theme::text())
                    .border_color(theme::text_muted())
            })
            .child(label)
    }
}

fn section(title: &'static str, hint: &'static str, body: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_sm().text_color(theme::text()).child(title))
                .child(div().text_xs().text_color(theme::text_muted()).child(hint)),
        )
        .child(body)
}

fn divider() -> Div {
    div().h(px(1.0)).bg(theme::border())
}

fn stat(label: &'static str, value: String) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(theme::text_muted()).child(label))
        .child(div().text_sm().text_color(theme::text()).child(value))
}

fn keycap(label: String) -> Div {
    div()
        .px_5()
        .py_3()
        .rounded_lg()
        .bg(theme::elevated())
        .border_1()
        .border_color(theme::border())
        .text_xl()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme::text())
        .child(label)
}

fn level_bars(
    level: f32,
    count: usize,
    max_height: f32,
    width: f32,
    phase: f32,
    color: Rgba,
) -> Div {
    let amplitude = (level.sqrt() * 1.2).clamp(0.0, 1.0);
    let bars = (0..count)
        .map(|index| {
            let wave = ((phase + index as f32 * 0.55).sin() * 0.5 + 0.5).powi(2);
            let height = 4.0 + wave * (6.0 + amplitude * (max_height - 10.0));
            div().w(px(width)).h(px(height)).rounded_full().bg(color)
        })
        .collect::<Vec<_>>();
    div()
        .flex()
        .items_center()
        .gap_1()
        .h(px(max_height))
        .children(bars)
}

fn model_label(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Local model");
    let lower = stem.to_ascii_lowercase();
    if lower.contains("large-v3-turbo") {
        "Whisper large-v3-turbo".to_owned()
    } else if lower.contains("large-v3") {
        "Whisper large-v3".to_owned()
    } else {
        stem.to_owned()
    }
}

fn model_quantization(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    stem.rsplit_once('-')
        .map(|(_, quantization)| quantization.to_ascii_uppercase())
        .filter(|quantization| !quantization.is_empty())
        .unwrap_or_else(|| "GGUF".to_owned())
}

fn preview(text: &str) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(140).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}
