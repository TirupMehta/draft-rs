//! The one Draft window: live state, the dictation loop, and the engine
//! hand-off. The model lives on its own thread; this view only polls events.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use draft_audio::Recorder;
use draft_core::settings::Settings;
use draft_engine::{ComputeBackend, EngineTimings};
use draft_hotkey::{HotkeyConfig, HotkeyEvent, HotkeyListener};
use gpui::{
    AnyElement, AnyWindowHandle, Context, Div, Entity, IntoElement, Render, Rgba, Stateful,
    WeakEntity, Window, WindowAppearance, WindowControlArea, div, prelude::*, px, size,
};
use tray_icon::menu::MenuEvent;
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconEvent};

use crate::hud::{DraftHud, HudSnapshot};
use crate::motion::Motion;
use crate::startup;
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
/// Stable names for the controls that ease. A control is identified by a
/// static key so the animation state survives a repaint.
const HOTKEY_KEYS: [&str; 6] = [
    "chip:hotkey:0",
    "chip:hotkey:1",
    "chip:hotkey:2",
    "chip:hotkey:3",
    "chip:hotkey:4",
    "chip:hotkey:5",
];
const MICROPHONE_KEYS: [&str; 8] = [
    "chip:microphone:0",
    "chip:microphone:1",
    "chip:microphone:2",
    "chip:microphone:3",
    "chip:microphone:4",
    "chip:microphone:5",
    "chip:microphone:6",
    "chip:microphone:7",
];
const SECTION_KEYS: [&str; 6] = [
    "section:0",
    "section:1",
    "section:2",
    "section:3",
    "section:4",
    "section:5",
];
const SECTION_HOVER_KEYS: [&str; 6] = [
    "section-hover:0",
    "section-hover:1",
    "section-hover:2",
    "section-hover:3",
    "section-hover:4",
    "section-hover:5",
];
const LEVEL_TEST_LENGTH: Duration = Duration::from_secs(4);
/// The shortest gap between preview passes. A preview is display-only, so a
/// little less freshness is a good trade for a much lower, steadier GPU load.
const PREVIEW_BASE_DELAY: Duration = Duration::from_millis(1_200);
const PREVIEW_MAX_DELAY: Duration = Duration::from_millis(3_000);
/// The HUD's level meter is decorative, not an input device. Fifteen frames a
/// second is plenty for a small capsule and keeps the GPU from redrawing it at
/// the full event-poll rate.
const HUD_INTERVAL: Duration = Duration::from_millis(66);
/// Cap repaints of the visible main window while its level meter is moving.
/// The event poll stays faster than this so a hotkey release is not delayed.
const UI_INTERVAL: Duration = Duration::from_millis(33);
/// Level samples kept for the recording meter. The app only knows the current
/// microphone level, so this is a real amplitude history and nothing more.
const ENVELOPE_SAMPLES: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Loading,
    Ready,
    Listening,
    Transcribing,
    Error,
}

/// What the window is showing. The everyday surface is small; configuration
/// gets its own, larger layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Dictate,
    Settings,
}

/// Settings are grouped by intent, not by implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Dictation,
    Audio,
    Recognition,
    Appearance,
    Privacy,
    Advanced,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Section::Dictation => "Dictation",
            Section::Audio => "Audio",
            Section::Recognition => "Recognition",
            Section::Appearance => "Appearance",
            Section::Privacy => "Privacy",
            Section::Advanced => "Advanced",
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Section::Dictation => "How a dictation starts, ends, and reaches the page.",
            Section::Audio => "Which microphone Draft listens to.",
            Section::Recognition => "The model that turns speech into text.",
            Section::Appearance => "How Draft looks on this screen.",
            Section::Privacy => "What stays on this device, and the one exception.",
            Section::Advanced => "Diagnostics, storage, and the command line.",
        }
    }

    fn all() -> [Section; 6] {
        [
            Section::Dictation,
            Section::Audio,
            Section::Recognition,
            Section::Appearance,
            Section::Privacy,
            Section::Advanced,
        ]
    }

    fn index(self) -> usize {
        match self {
            Section::Dictation => 0,
            Section::Audio => 1,
            Section::Recognition => 2,
            Section::Appearance => 3,
            Section::Privacy => 4,
            Section::Advanced => 5,
        }
    }
}

/// The cross-fade between the everyday view and the settings view. The
/// window changes size in the middle of the fade, where nothing is visible,
/// so the change of layout never arrives as a jump.
struct ViewTransition {
    from: View,
    to: View,
    started: Instant,
    swapped: bool,
}

const FADE_OUT: Duration = Duration::from_millis(70);
const FADE_IN: Duration = Duration::from_millis(140);

/// The size of each view.
fn view_size(view: View) -> (f32, f32) {
    match view {
        View::Dictate => (560.0, 240.0),
        View::Settings => (680.0, 500.0),
    }
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
    /// Wait at least this long after a finished preview before starting the
    /// next one. It grows with the measured preview cost, which keeps the GPU
    /// duty cycle bounded instead of running previews back-to-back.
    preview_delay: Duration,
    serial: u64,
    last: Option<LastResult>,
    error: Option<String>,
    notice: Option<String>,
    model: PathBuf,
    settings: Settings,
    settings_path: PathBuf,
    /// Whether finished transcripts are inserted at the cursor. A command line
    /// flag can turn it off for one launch; the settings switch owns it after
    /// that.
    paste_enabled: bool,
    history: Option<draft_core::history::History>,
    hud: Entity<DraftHud>,
    hud_snapshot: Option<HudSnapshot>,
    hud_at: Instant,
    ui_at: Instant,
    hud_visible: bool,
    hud_hwnd: Option<isize>,
    main_window: Option<AnyWindowHandle>,
    main_hwnd: Option<isize>,
    tray: Option<TrayIcon>,
    clock: Instant,
    view: View,
    section: Section,
    /// A short history of the real microphone level. It is the only waveform
    /// data Draft has, so the interface never invents one.
    envelope: VecDeque<f32>,
    /// When the last transcript was inserted, so the confirmation stays true.
    pasted_at: Option<Instant>,
    /// The palette in use. The HUD needs it outside a render pass, so the
    /// last resolved palette is kept here.
    palette: theme::Palette,
    /// Every control that eases instead of snapping.
    motion: Motion,
    /// The cross-fade between the two views, when one is running.
    transition: Option<ViewTransition>,
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
        // Keep the Run entry pointed at this executable after an in-place
        // update. Debug builds never touch the user's startup settings.
        #[cfg(all(target_os = "windows", not(debug_assertions)))]
        if let Err(error) = startup::apply(settings.launch_at_startup) {
            tracing::warn!(%error, "could not update the startup registration");
        }

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
        let (jobs, worker_events) = worker::spawn(model, backend);
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
            preview_delay: PREVIEW_BASE_DELAY,
            serial: 0,
            last: None,
            error: initial_error,
            notice,
            model: model_display,
            settings,
            settings_path,
            paste_enabled: paste,
            history,
            hud,
            hud_snapshot: None,
            hud_at: Instant::now(),
            ui_at: Instant::now(),
            hud_visible: false,
            hud_hwnd: None,
            main_window: None,
            main_hwnd: None,
            tray,
            clock: Instant::now(),
            view: View::Dictate,
            section: Section::Dictation,
            envelope: VecDeque::new(),
            pasted_at: None,
            palette: theme::Palette::dark(),
            motion: Motion::new(if window_control::system_animations_enabled() {
                std::time::Duration::from_millis(150)
            } else {
                std::time::Duration::ZERO
            }),
            transition: None,
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
        // A phase, transcript, or notice change should reach the HUD at once.
        // Level-only updates are throttled separately below.
        let state_changed = changed;
        let ui_due = state_changed || self.ui_at.elapsed() >= UI_INTERVAL;
        let main_visible = self.main_hwnd.is_none_or(window_control::is_visible);
        if self.recorder.is_some() {
            // Keep a short history of the real microphone level. It is the
            // only waveform data Draft has, and it costs one number per poll.
            self.envelope.push_back(self.input_level());
            while self.envelope.len() > ENVELOPE_SAMPLES {
                self.envelope.pop_front();
            }
        }
        if (self.recorder.is_some() || self.level_test.is_some()) && main_visible {
            // Keep the visible window's level meter moving while audio is
            // open. When Draft is hidden in the tray, the HUD is the only
            // thing that needs a repaint.
            changed = true;
        }
        if self.pending_jobs > 0 && main_visible {
            // Inference is short. The window says it is working, so the state
            // animates while the model runs.
            changed = true;
        }
        // The view cross-fade: fade out, swap and resize while nothing is
        // visible, then fade the new view in.
        let mut swap_view = false;
        if let Some(transition) = self.transition.as_ref() {
            if transition.swapped {
                if transition.started.elapsed() >= FADE_IN {
                    self.transition = None;
                }
            } else if transition.started.elapsed() >= FADE_OUT {
                swap_view = true;
            }
        }
        if swap_view {
            if let Some(target) = self.transition.as_ref().map(|transition| transition.to) {
                if let Some(transition) = self.transition.as_mut() {
                    transition.swapped = true;
                    transition.started = Instant::now();
                }
                self.view = target;
                // The window is resized through the framework, so the drawing
                // surface follows it. It happens while nothing is visible.
                let (width, height) = view_size(target);
                if let Some(handle) = self.main_window {
                    let _ = handle.update(cx, |_, window, _| {
                        window.resize(size(px(width), px(height)));
                    });
                }
            }
        }
        if self.transition.is_some() {
            changed = true;
        }
        if self.motion.advance() {
            changed = true;
        }
        self.maybe_preview();
        if state_changed || self.hud_at.elapsed() >= HUD_INTERVAL {
            self.sync_hud(cx);
            self.hud_at = Instant::now();
        }
        if changed && ui_due {
            cx.notify();
            self.ui_at = Instant::now();
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

    fn set_live_preview(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.settings.live_preview == enabled {
            return;
        }
        self.settings.live_preview = enabled;
        if enabled {
            // Start the next preview clock fresh when the user opts in.
            self.preview_at = Instant::now();
            self.preview_delay = PREVIEW_BASE_DELAY;
            self.notice = Some("Live preview on. This uses the GPU while you speak.".to_owned());
        } else {
            // Drop any partial already on screen; a preview still in flight is
            // ignored when it arrives.
            self.partial = None;
            self.notice = Some("Live preview off. Dictation uses one final GPU pass.".to_owned());
        }
        self.save_settings();
        cx.notify();
    }

    fn set_launch_at_startup(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.settings.launch_at_startup == enabled {
            return;
        }
        match startup::apply(enabled) {
            Ok(()) => {
                self.settings.launch_at_startup = enabled;
                if self.save_settings() {
                    self.notice = Some(if enabled {
                        "Draft will start in the tray when you sign in.".to_owned()
                    } else {
                        "Draft will no longer start when you sign in.".to_owned()
                    });
                }
            }
            Err(error) => {
                tracing::warn!(%error, "could not update the startup registration");
                self.notice = Some(format!("Could not update startup: {error}"));
            }
        }
        cx.notify();
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
        let text = draft_core::text::for_paste(&last.text);
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
                self.palette,
            )),
            Phase::Transcribing => Some(HudSnapshot::transcribing(self.palette)),
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
        if !self.settings.live_preview || self.phase != Phase::Listening || self.preview_pending {
            return;
        }
        if self.preview_at.elapsed() < self.preview_delay {
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
        // Whisper invents words on pure silence, so only ask for a preview
        // when the tail actually carries voice-level energy.
        let rms = (recording
            .samples
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / recording.samples.len() as f32)
            .sqrt();
        if rms < 0.0015 {
            self.preview_at = Instant::now();
            return;
        }
        let job = Job {
            samples: recording.samples,
            serial: self.serial,
            preview: true,
            paste: false,
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
            worker::Event::Partial {
                text,
                serial,
                elapsed,
            } => {
                if serial != self.serial {
                    self.preview_pending = false;
                    self.preview_at = Instant::now();
                    return;
                }
                self.preview_pending = false;
                self.preview_at = Instant::now();
                // Hold the next preview off for a few times the cost of this
                // one. The gap has a floor so previews still feel live, and a
                // ceiling so a slow pass does not stall the HUD forever.
                self.preview_delay = elapsed
                    .saturating_mul(3)
                    .clamp(PREVIEW_BASE_DELAY, PREVIEW_MAX_DELAY);
                if self.settings.live_preview
                    && self.phase == Phase::Listening
                    && !text.trim().is_empty()
                {
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
                tracing::info!(
                    audio_s,
                    engine_ms = elapsed.as_millis(),
                    chars = text.len(),
                    "dictation transcribed"
                );
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
                if self
                    .last
                    .as_ref()
                    .is_some_and(|last| last.paste_error.is_none())
                {
                    self.pasted_at = Some(Instant::now());
                }
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
        self.preview_delay = PREVIEW_BASE_DELAY;
        self.serial = self.serial.wrapping_add(1);
        self.envelope.clear();
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
                    paste: self.paste_enabled,
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

    /// The palette for this frame. In `System` mode the window follows the
    /// operating system, including live system changes.
    fn palette_for(&self, window: &Window) -> theme::Palette {
        match theme::Appearance::from_setting(&self.settings.appearance) {
            theme::Appearance::Light => theme::Palette::light(),
            theme::Appearance::Dark => theme::Palette::dark(),
            theme::Appearance::System => match window.appearance() {
                WindowAppearance::Light | WindowAppearance::VibrantLight => theme::Palette::light(),
                _ => theme::Palette::dark(),
            },
        }
    }

    fn set_view(&mut self, view: View, _window: &mut Window, cx: &mut Context<Self>) {
        if self.view == view || self.transition.is_some() {
            return;
        }
        // The old view fades out, the window changes size while nothing is
        // visible, and the new view fades in. Nothing lands as a jump.
        self.transition = Some(ViewTransition {
            from: self.view,
            to: view,
            started: Instant::now(),
            swapped: false,
        });
        self.motion.set_hover(
            "view:dictate",
            if view == View::Dictate { 1.0 } else { 0.0 },
        );
        self.motion.set_hover(
            "view:settings",
            if view == View::Settings { 1.0 } else { 0.0 },
        );
        cx.notify();
    }

    fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.section == section {
            return;
        }
        self.section = section;
        for (index, key) in SECTION_KEYS.iter().enumerate() {
            let selected = index == section.index();
            self.motion.set_hover(key, if selected { 1.0 } else { 0.0 });
        }
        self.motion.restart("section:fade", 1.0);
        cx.notify();
    }

    fn open_section(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        self.section = section;
        self.set_view(View::Settings, window, cx);
    }

    fn set_appearance(
        &mut self,
        appearance: theme::Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings.appearance == appearance.id() {
            return;
        }
        self.settings.appearance = appearance.id().to_owned();
        self.save_settings();
        // The native title bar belongs to Windows, so it has to be told which
        // appearance the content uses.
        let dark = match appearance {
            theme::Appearance::Dark => true,
            theme::Appearance::Light => false,
            theme::Appearance::System => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };
        if let Some(hwnd) = self.main_hwnd {
            window_control::set_title_bar_dark(hwnd, dark);
        }
        cx.notify();
    }

    fn set_paste(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.settings.paste == enabled {
            return;
        }
        self.settings.paste = enabled;
        self.paste_enabled = enabled;
        self.save_settings();
        self.notice = Some(if enabled {
            "Transcripts are pasted at the cursor.".to_owned()
        } else {
            "Transcripts stay in Draft. Use Paste last to insert one.".to_owned()
        });
        cx.notify();
    }

    fn copy_last(&mut self, cx: &mut Context<Self>) {
        let Some(last) = self.last.as_ref() else {
            self.notice = Some("No completed dictation yet.".to_owned());
            cx.notify();
            return;
        };
        let text = last.text.clone();
        std::thread::spawn(move || {
            if let Err(error) = draft_paste::copy_text(&text) {
                tracing::warn!(%error, "could not copy the last dictation");
            }
        });
        self.notice = Some("Copied to the clipboard.".to_owned());
        cx.notify();
    }

    fn clear_history(&mut self, cx: &mut Context<Self>) {
        let Some(history) = self.history.as_mut() else {
            self.notice = Some("Local history is unavailable.".to_owned());
            cx.notify();
            return;
        };
        match history.clear() {
            Ok(()) => self.notice = Some("Local history cleared.".to_owned()),
            Err(error) => {
                self.notice = Some(format!("Could not clear the history: {error:#}"));
            }
        }
        cx.notify();
    }

    fn open_data_folder(&mut self, cx: &mut Context<Self>) {
        open_in_file_manager(&draft_core::paths::data_dir());
        self.notice = Some("Opened the Draft data folder.".to_owned());
        cx.notify();
    }

    fn open_model_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.model.parent() {
            open_in_file_manager(parent);
        }
        cx.notify();
    }

    fn open_logs_folder(&mut self, cx: &mut Context<Self>) {
        open_in_file_manager(&draft_core::paths::data_dir().join("logs"));
        self.notice = Some("Opened the Draft log folder.".to_owned());
        cx.notify();
    }
}

impl DraftApp {
    /// The everyday surface. One focal state, one line of guidance, and
    /// nothing else competing for attention.
    fn dictate_view(&mut self, palette: &theme::Palette, cx: &mut Context<Self>) -> Div {
        let state = self.state_block(palette, cx);
        let mut below = div().flex().flex_col().gap(px(theme::space::SM)).w_full();
        if let Some(last) = &self.last {
            below = below.child(self.result_block(palette, last, cx));
        }
        if let Some(notice) = &self.notice {
            below = below.child(
                div()
                    .text_size(px(theme::type_size::SMALL))
                    .text_color(palette.tertiary)
                    .child(notice.clone()),
            );
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.title_bar(palette, cx))
            .child(div().h(px(1.0)).bg(palette.hairline))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .flex_col()
                    .px(px(theme::space::XL))
                    .py(px(theme::space::LG))
                    .gap(px(theme::space::LG))
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .flex()
                            .flex_col()
                            .justify_center()
                            .child(state),
                    )
                    .child(below)
                    .child(self.footer(palette)),
            )
    }

    /// The one thing the window is saying right now.
    fn state_block(&mut self, palette: &theme::Palette, cx: &mut Context<Self>) -> Div {
        let mut parts: Vec<AnyElement> = Vec::new();
        match self.phase {
            Phase::Loading => {
                parts.push(headline("Warming the model", palette.text).into_any_element());
                parts.push(
                    secondary(
                        palette,
                        "The local model loads once and stays ready on this device.",
                    )
                    .into_any_element(),
                );
            }
            Phase::Ready => {
                parts.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(theme::space::SM))
                        .child(headline("Hold", palette.text))
                        .child(keycap(palette, &self.hotkey_label))
                        .child(headline("and speak", palette.text))
                        .into_any_element(),
                );
                parts
                    .push(secondary(palette, "Release to paste at the cursor.").into_any_element());
            }
            Phase::Listening => {
                parts.push(headline("Listening", palette.accent_text).into_any_element());
                if let Some(partial) = self.partial.as_deref() {
                    parts.push(
                        div()
                            .text_size(px(theme::type_size::TITLE))
                            .text_color(palette.text)
                            .line_height(px(22.0))
                            .child(partial_preview(partial))
                            .into_any_element(),
                    );
                }
                parts.push(self.meter(palette).into_any_element());
                parts.push(
                    secondary(
                        palette,
                        &format!(
                            "Release {} to transcribe · Esc to cancel",
                            self.hotkey_label
                        ),
                    )
                    .into_any_element(),
                );
            }
            Phase::Transcribing => {
                parts.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(theme::space::SM))
                        .child(headline("Transcribing", palette.text))
                        .child(self.ellipsis(palette))
                        .into_any_element(),
                );
                parts.push(secondary(palette, "Finishing on this device.").into_any_element());
            }
            Phase::Error => {
                parts
                    .push(headline("Something needs attention", palette.danger).into_any_element());
                parts.push(
                    secondary(
                        palette,
                        &preview(
                            self.error
                                .as_deref()
                                .unwrap_or("Draft could not start dictation."),
                        ),
                    )
                    .into_any_element(),
                );
                parts.push(
                    div()
                        .id("error-audio-settings")
                        .text_size(px(theme::type_size::BODY))
                        .text_color(palette.accent_text)
                        .cursor_pointer()
                        .hover(|style| style.text_color(palette.text))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_section(Section::Audio, window, cx)
                        }))
                        .child("Open audio settings")
                        .into_any_element(),
                );
            }
        }
        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::MD))
            .items_start()
            .children(parts)
    }

    /// The last transcript. It is the confirmation: the text is what was
    /// inserted, and the line under it says when.
    fn result_block(
        &self,
        palette: &theme::Palette,
        last: &LastResult,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut block = div()
            .flex()
            .flex_col()
            .gap(px(theme::space::SM))
            .w_full()
            .child(div().h(px(1.0)).bg(palette.hairline))
            .child(
                div()
                    .text_size(px(theme::type_size::BODY))
                    .line_height(px(19.0))
                    .text_color(palette.text)
                    .h(px(38.0))
                    .overflow_hidden()
                    .child(preview(&last.text)),
            );
        if let Some(error) = &last.paste_error {
            block = block.child(
                div()
                    .text_size(px(theme::type_size::SMALL))
                    .text_color(palette.danger)
                    .child(format!("Could not paste: {error}")),
            );
        } else {
            let when = match self.pasted_at {
                Some(at) => pasted_ago(at),
                None => "Transcript ready".to_owned(),
            };
            block = block.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(theme::space::MD))
                    .child(
                        div()
                            .text_size(px(theme::type_size::META))
                            .text_color(palette.tertiary)
                            .child(when),
                    )
                    .child(
                        div()
                            .id("copy-last")
                            .text_size(px(theme::type_size::META))
                            .text_color(palette.secondary)
                            .cursor_pointer()
                            .hover(|style| style.text_color(palette.text))
                            .on_click(cx.listener(|this, _, _, cx| this.copy_last(cx)))
                            .child("Copy"),
                    ),
            );
        }
        block
    }

    /// The real microphone level over the last few seconds. Draft only knows
    /// one number, so this is an amplitude history and not a drawn wave.
    fn meter(&self, palette: &theme::Palette) -> Div {
        let taken: Vec<f32> = self
            .envelope
            .iter()
            .rev()
            .take(ENVELOPE_SAMPLES)
            .rev()
            .copied()
            .collect();
        let mut samples = vec![0.0_f32; ENVELOPE_SAMPLES];
        let offset = ENVELOPE_SAMPLES - taken.len();
        samples[offset..].copy_from_slice(&taken);

        let bars = samples.into_iter().map(|level| {
            let amplitude = (level.sqrt() * 1.15).clamp(0.0, 1.0);
            let height = 2.0 + amplitude * 26.0;
            div()
                .w(px(3.0))
                .h(px(height))
                .rounded_full()
                .bg(if amplitude > 0.02 {
                    palette.accent
                } else {
                    palette.track
                })
        });
        div()
            .flex()
            .items_end()
            .gap(px(3.0))
            .h(px(30.0))
            .children(bars)
    }

    /// A quiet sign that work is happening. It never claims a percentage.
    fn ellipsis(&self, palette: &theme::Palette) -> Div {
        let phase = self.clock.elapsed().as_secs_f32() * 2.6;
        let dots = (0..3).map(|index| {
            let wave = ((phase - index as f32 * 0.3).sin() * 0.5 + 0.5).powi(2);
            div()
                .w(px(4.0))
                .h(px(4.0))
                .rounded_full()
                .bg(if wave > 0.45 {
                    palette.secondary
                } else {
                    palette.tertiary
                })
        });
        div().flex().items_center().gap(px(3.0)).children(dots)
    }

    /// The window's own title bar. Draft draws it inside the window so the
    /// chrome and the body are one surface, instead of a system frame stacked
    /// on top of an application header.
    fn title_bar(&self, palette: &theme::Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let app = cx.entity().downgrade();
        div()
            .id("title-bar")
            .h(px(44.0))
            .flex()
            .items_center()
            .justify_between()
            .px(px(theme::space::SM))
            .pl(px(theme::space::LG))
            .child(
                div()
                    .id("title-bar-brand")
                    .flex()
                    .items_center()
                    .gap(px(theme::space::SM))
                    .window_control_area(WindowControlArea::Drag)
                    .child(brand_mark(palette, 16.0))
                    .child(
                        div()
                            .font_family(theme::display_font())
                            .text_size(px(theme::type_size::BODY))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(palette.text)
                            .child("Draft"),
                    ),
            )
            .child(
                div()
                    .id("title-bar-drag")
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(self.view_control(&app, palette, cx))
            .child(self.window_buttons(&app, palette, cx))
    }

    /// The everyday view and the settings view, as one control.
    /// A selectable option. Selection is a quiet wash, hover eases in, and a
    /// press settles it a little darker.
    fn chip(
        &self,
        app: &WeakEntity<DraftApp>,
        palette: &theme::Palette,
        key: &'static str,
        label: impl IntoElement,
        selected: bool,
    ) -> Stateful<Div> {
        let hover = self.motion.hover(key);
        let press = self.motion.press(key);
        let resting = if selected {
            palette.accent_soft
        } else {
            theme::transparent()
        };
        let hovered = if selected {
            theme::mix(palette.accent_soft, palette.raised_hover, 0.65)
        } else {
            palette.raised_hover
        };
        let resting_text = if selected {
            palette.text
        } else {
            palette.secondary
        };
        let background = theme::mix(resting, hovered, hover);
        let background = theme::mix(background, palette.canvas, press * 0.45);
        let element = div()
            .id(key)
            .px(px(10.0))
            .py(px(5.0))
            .rounded_md()
            .bg(background)
            .text_size(px(theme::type_size::SMALL))
            .text_color(theme::mix(resting_text, palette.text, hover))
            .cursor_pointer()
            .child(label);
        pressable(app.clone(), key, hoverable(app.clone(), key, element))
    }

    /// A quiet text action, such as "Copy", "Open", or "Test".
    fn text_button(
        &self,
        app: &WeakEntity<DraftApp>,
        palette: &theme::Palette,
        key: &'static str,
        label: impl IntoElement,
    ) -> Stateful<Div> {
        let hover = self.motion.hover(key);
        let press = self.motion.press(key);
        let background = theme::mix(theme::transparent(), palette.raised_hover, hover);
        let background = theme::mix(background, palette.canvas, press * 0.45);
        let element = div()
            .id(key)
            .px(px(8.0))
            .py(px(4.0))
            .rounded_md()
            .bg(background)
            .text_size(px(theme::type_size::SMALL))
            .text_color(theme::mix(palette.secondary, palette.text, hover))
            .cursor_pointer()
            .child(label);
        pressable(app.clone(), key, hoverable(app.clone(), key, element))
    }

    /// A switch. The knob is the only part that moves.
    fn toggle(
        &self,
        app: &WeakEntity<DraftApp>,
        palette: &theme::Palette,
        key: &'static str,
        checked: bool,
    ) -> Stateful<Div> {
        let hover = self.motion.hover(key);
        let press = self.motion.press(key);
        let track = if checked {
            palette.accent
        } else {
            theme::mix(palette.raised, palette.raised_hover, hover)
        };
        let track = theme::mix(track, palette.canvas, press * 0.35);
        let knob = if checked {
            theme::white()
        } else {
            theme::mix(palette.tertiary, palette.secondary, hover)
        };
        let element = div()
            .id(key)
            .w(px(38.0))
            .h(px(22.0))
            .rounded_full()
            .bg(track)
            .border_1()
            .border_color(if checked {
                palette.accent
            } else {
                palette.hairline
            })
            .flex()
            .items_center()
            .px(px(3.0))
            .cursor_pointer()
            .child(div().w(px(16.0)).h(px(16.0)).rounded_full().bg(knob));
        let element = if checked {
            element.justify_end()
        } else {
            element.justify_start()
        };
        pressable(app.clone(), key, hoverable(app.clone(), key, element))
    }
    fn view_control(
        &self,
        app: &WeakEntity<DraftApp>,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .p(px(2.0))
            .rounded_md()
            .bg(palette.raised)
            .child(self.view_tab(app, palette, View::Dictate, cx))
            .child(self.view_tab(app, palette, View::Settings, cx))
    }

    /// Minimize, maximize, and close. Close hides Draft to the tray, which is
    /// what the system frame used to do.
    fn window_buttons(
        &self,
        app: &WeakEntity<DraftApp>,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let minimize = div()
            .id("window-minimize")
            .w(px(34.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::mix(
                theme::transparent(),
                palette.raised_hover,
                self.motion.hover("window:minimize"),
            ))
            .cursor_pointer()
            .on_click(cx.listener(|_, _, window, _| {
                window.minimize_window();
            }))
            .child(div().w(px(10.0)).h(px(1.0)).bg(theme::mix(
                palette.secondary,
                palette.text,
                self.motion.hover("window:minimize"),
            )));

        let maximize = div()
            .id("window-maximize")
            .w(px(34.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::mix(
                theme::transparent(),
                palette.raised_hover,
                self.motion.hover("window:maximize"),
            ))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, _| {
                if let Some(hwnd) = this.main_hwnd {
                    window_control::toggle_maximize(hwnd);
                }
            }))
            .child(
                div()
                    .w(px(10.0))
                    .h(px(10.0))
                    .border_1()
                    .border_color(theme::mix(
                        palette.secondary,
                        palette.text,
                        self.motion.hover("window:maximize"),
                    )),
            );

        let close_hover = self.motion.hover("window:close");
        let close = div()
            .id("window-close")
            .w(px(34.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::mix(
                theme::transparent(),
                palette.danger,
                close_hover,
            ))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| {
                if this.tray.is_some() {
                    window_control::hide(window);
                } else {
                    cx.quit();
                }
            }))
            .child(
                div()
                    .text_size(px(13.0))
                    .line_height(px(13.0))
                    .text_color(theme::mix(palette.secondary, theme::white(), close_hover))
                    .child("✕"),
            );

        div()
            .flex()
            .items_center()
            .child(hoverable(
                app.clone(),
                "window:minimize",
                pressable(app.clone(), "window:minimize", minimize),
            ))
            .child(hoverable(
                app.clone(),
                "window:maximize",
                pressable(app.clone(), "window:maximize", maximize),
            ))
            .child(hoverable(
                app.clone(),
                "window:close",
                pressable(app.clone(), "window:close", close),
            ))
    }

    fn view_tab(
        &self,
        app: &WeakEntity<DraftApp>,
        palette: &theme::Palette,
        view: View,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (selection_key, hover_key, label) = match view {
            View::Dictate => ("view:dictate", "tab:dictate", "Dictate"),
            View::Settings => ("view:settings", "tab:settings", "Settings"),
        };
        let selected = self.motion.hover(selection_key);
        let hover = self.motion.hover(hover_key);
        let element = div()
            .id((
                "view-tab",
                if view == View::Dictate {
                    0usize
                } else {
                    1usize
                },
            ))
            .px(px(12.0))
            .py(px(5.0))
            .rounded_sm()
            .bg(theme::mix(
                theme::transparent(),
                palette.raised_hover,
                selected.max(hover * 0.5),
            ))
            .text_size(px(theme::type_size::SMALL))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme::mix(
                palette.secondary,
                palette.text,
                selected.max(hover),
            ))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| this.set_view(view, window, cx)))
            .child(label);
        pressable(
            app.clone(),
            hover_key,
            hoverable(app.clone(), hover_key, element),
        )
    }
    fn footer(&self, palette: &theme::Palette) -> Div {
        let engine = if self.engine_ready {
            engine_label(self.backend.as_deref(), self.device.as_deref())
        } else {
            "Preparing model".to_owned()
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(theme::type_size::META))
                    .text_color(palette.tertiary)
                    .child("Your voice stays on this device."),
            )
            .child(
                div()
                    .text_size(px(theme::type_size::META))
                    .text_color(palette.tertiary)
                    .child(engine),
            )
    }

    /// Configuration lives in its own space, grouped by what a person is
    /// trying to change rather than by how the app is built.
    fn settings_view(&mut self, palette: &theme::Palette, cx: &mut Context<Self>) -> Div {
        let app = cx.entity().downgrade();
        let mut rail = div()
            .w(px(150.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .px(px(theme::space::LG))
            .py(px(theme::space::LG));
        for section in Section::all() {
            rail = rail.child(self.section_button(&app, palette, section, cx));
        }
        let pane = self.section_pane(palette, cx);
        // A section change dissolves instead of cutting.
        let fade = 0.25 + 0.75 * self.motion.hover("section:fade");

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.title_bar(palette, cx))
            .child(div().h(px(1.0)).bg(palette.hairline))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(rail)
                    .child(div().w(px(1.0)).bg(palette.hairline))
                    .child(
                        div()
                            .id("settings-pane")
                            .flex_1()
                            .min_w(px(0.0))
                            .px(px(theme::space::XL))
                            .py(px(theme::space::LG))
                            .overflow_y_scroll()
                            .child(pane.opacity(fade)),
                    ),
            )
    }

    fn section_button(
        &self,
        app: &WeakEntity<DraftApp>,
        palette: &theme::Palette,
        section: Section,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let index = section.index();
        let selected = self.motion.hover(SECTION_KEYS[index]);
        let hover = self.motion.hover(SECTION_HOVER_KEYS[index]);
        let element = div()
            .id(("section", index))
            .flex()
            .items_center()
            .gap(px(theme::space::SM))
            .px(px(theme::space::SM))
            .py(px(5.0))
            .rounded_md()
            .bg(theme::mix(
                theme::transparent(),
                palette.raised_hover,
                hover * 0.55,
            ))
            .text_size(px(theme::type_size::BODY))
            .text_color(theme::mix(
                palette.secondary,
                palette.text,
                selected.max(hover),
            ))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.set_section(section, cx)))
            .child(
                div()
                    .w(px(2.0))
                    .h(px(12.0))
                    .rounded_full()
                    .bg(palette.accent)
                    .opacity(selected),
            )
            .child(section.title());
        pressable(
            app.clone(),
            SECTION_HOVER_KEYS[index],
            hoverable(app.clone(), SECTION_HOVER_KEYS[index], element),
        )
    }

    fn section_pane(&mut self, palette: &theme::Palette, cx: &mut Context<Self>) -> Div {
        let section = self.section;
        let mut pane = div().flex().flex_col();
        pane = pane.child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .pb(px(theme::space::MD))
                .child(
                    div()
                        .font_family(theme::display_font())
                        .text_size(px(theme::type_size::TITLE))
                        .line_height(px(20.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(palette.text)
                        .child(section.title()),
                )
                .child(
                    div()
                        .text_size(px(theme::type_size::SMALL))
                        .line_height(px(16.0))
                        .text_color(palette.secondary)
                        .child(section.summary()),
                ),
        );

        let rows: Vec<AnyElement> = match section {
            Section::Dictation => self.dictation_rows(palette, cx),
            Section::Audio => self.audio_rows(palette, cx),
            Section::Recognition => self.recognition_rows(palette, cx),
            Section::Appearance => self.appearance_rows(palette, cx),
            Section::Privacy => self.privacy_rows(palette, cx),
            Section::Advanced => self.advanced_rows(palette, cx),
        };
        for (index, row) in rows.into_iter().enumerate() {
            if index > 0 {
                pane = pane.child(div().h(px(1.0)).bg(palette.hairline));
            }
            pane = pane.child(row);
        }
        pane
    }

    fn dictation_rows(
        &mut self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let app = cx.entity().downgrade();
        let hotkeys = div().flex().flex_wrap().gap(px(theme::space::SM)).children(
            HOTKEYS.iter().enumerate().filter_map(|(index, name)| {
                let config = HotkeyConfig::named(name)?;
                Some(
                    self.chip(
                        &app,
                        palette,
                        HOTKEY_KEYS[index],
                        config.name,
                        self.hotkey == config,
                    )
                    .id(("hotkey", index))
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_hotkey(config, cx))),
                )
            }),
        );

        let live_preview = self
            .toggle(&app, palette, "toggle:preview", self.settings.live_preview)
            .on_click(cx.listener(|this, _, _, cx| {
                let enabled = !this.settings.live_preview;
                this.set_live_preview(enabled, cx);
            }));
        let paste = self
            .toggle(&app, palette, "toggle:paste", self.settings.paste)
            .on_click(cx.listener(|this, _, _, cx| {
                let enabled = !this.settings.paste;
                this.set_paste(enabled, cx);
            }));
        let startup = self
            .toggle(
                &app,
                palette,
                "toggle:startup",
                self.settings.launch_at_startup,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                let enabled = !this.settings.launch_at_startup;
                this.set_launch_at_startup(enabled, cx);
            }));

        vec![
            setting_block(
                palette,
                "Dictation key",
                "Hold to speak. Release to insert the text.",
                hotkeys,
            )
            .into_any_element(),
            setting_row(
                palette,
                "Live preview",
                "Show words while you speak. Costs extra GPU time.",
                live_preview,
            )
            .into_any_element(),
            setting_row(
                palette,
                "Paste at the cursor",
                "Insert the transcript as soon as it is ready.",
                paste,
            )
            .into_any_element(),
            setting_row(
                palette,
                "Start at sign-in",
                "Launch Draft in the tray when you sign in.",
                startup,
            )
            .into_any_element(),
        ]
    }

    fn audio_rows(&mut self, palette: &theme::Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let app = cx.entity().downgrade();
        let mut chips = vec![
            self.chip(
                &app,
                palette,
                "chip:microphone:default",
                "System default",
                self.microphone.is_none(),
            )
            .id("microphone-default")
            .on_click(cx.listener(|this, _, _, cx| this.choose_microphone(None, cx))),
        ];
        for (index, name) in self.microphones.iter().enumerate() {
            let selected = self.microphone.as_deref() == Some(name.as_str());
            let display = microphone_display(name);
            let label = if self.default_microphone.as_deref() == Some(name.as_str()) {
                format!("{display} · default")
            } else {
                display
            };
            let device = name.clone();
            let key = MICROPHONE_KEYS
                .get(index)
                .copied()
                .unwrap_or("chip:microphone:default");
            chips.push(
                self.chip(&app, palette, key, label, selected)
                    .id(("microphone", index))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose_microphone(Some(device.clone()), cx)
                    })),
            );
        }
        let level = (self.input_level().sqrt() * 1.25).clamp(0.0, 1.0);
        let meter = div()
            .w(px(120.0))
            .h(px(4.0))
            .rounded_full()
            .bg(palette.raised)
            .child(
                div()
                    .h_full()
                    .w(px(120.0 * level))
                    .rounded_full()
                    .bg(palette.accent),
            );
        let test = self
            .text_button(
                &app,
                palette,
                "button:test",
                if self.level_test.is_some() {
                    "Stop"
                } else {
                    "Test"
                },
            )
            .id("microphone-test")
            .on_click(cx.listener(|this, _, _, cx| this.test_microphone(cx)));

        vec![
            setting_block(
                palette,
                "Input device",
                "If the saved device disappears, Draft uses the system default.",
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(theme::space::SM))
                    .children(chips),
            )
            .into_any_element(),
            setting_row(
                palette,
                "Microphone test",
                "Speak for four seconds to check the level.",
                div()
                    .flex()
                    .items_center()
                    .gap(px(theme::space::MD))
                    .child(meter)
                    .child(test),
            )
            .into_any_element(),
        ]
    }

    fn recognition_rows(
        &mut self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let app = cx.entity().downgrade();
        let model_name = model_label(&self.model);
        let quantization = model_quantization(&self.model);
        let mut actions = Vec::new();
        if self.settings.model.is_some() {
            actions.push(
                self.text_button(&app, palette, "button:model-default", "Use default")
                    .id("model-default")
                    .on_click(cx.listener(|this, _, _, cx| this.use_default_model(cx))),
            );
        }
        actions.push(
            self.text_button(&app, palette, "button:model-folder", "Show in folder")
                .id("model-folder")
                .on_click(cx.listener(|this, _, _, cx| this.open_model_folder(cx))),
        );
        let backend = if self.engine_ready {
            engine_label(self.backend.as_deref(), self.device.as_deref())
        } else {
            "Preparing".to_owned()
        };
        let path = self.model.display().to_string();

        vec![
            setting_row(
                palette,
                "Model",
                &format!("{model_name} · {quantization} · local GGUF"),
                div()
                    .flex()
                    .items_center()
                    .gap(px(theme::space::MD))
                    .children(actions),
            )
            .into_any_element(),
            setting_block(
                palette,
                "Model file",
                "A local file. Draft never downloads a model while you dictate.",
                div()
                    .text_size(px(theme::type_size::META))
                    .line_height(px(15.0))
                    .text_color(palette.tertiary)
                    .child(path),
            )
            .into_any_element(),
            setting_row(
                palette,
                "Backend",
                "Where the model runs.",
                div()
                    .text_size(px(theme::type_size::SMALL))
                    .text_color(palette.secondary)
                    .child(backend),
            )
            .into_any_element(),
        ]
    }

    fn appearance_rows(
        &mut self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let app = cx.entity().downgrade();
        let options = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .p(px(2.0))
            .rounded_md()
            .bg(palette.raised)
            .children(theme::Appearance::all().into_iter().map(|appearance| {
                let selected = self.settings.appearance == appearance.id();
                let appearance_index = appearance.index();
                let key: &'static str = match appearance_index {
                    0 => "chip:theme:system",
                    1 => "chip:theme:light",
                    _ => "chip:theme:dark",
                };
                let hover = self.motion.hover(key);
                let element = div()
                    .id(("theme", appearance_index))
                    .px(px(12.0))
                    .py(px(5.0))
                    .rounded_sm()
                    .bg(theme::mix(
                        if selected {
                            palette.canvas
                        } else {
                            theme::transparent()
                        },
                        palette.raised_hover,
                        hover * 0.7,
                    ))
                    .text_size(px(theme::type_size::SMALL))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(if selected {
                        palette.text
                    } else {
                        palette.secondary
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_appearance(appearance, window, cx)
                    }))
                    .child(appearance.label());
                hoverable(app.clone(), key, element)
            }));

        vec![
            setting_row(
                palette,
                "Theme",
                "Follow the system, or choose light or dark.",
                options,
            )
            .into_any_element(),
        ]
    }

    fn privacy_rows(
        &mut self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let app = cx.entity().downgrade();
        let history_count = self
            .history
            .as_ref()
            .map(|history| history.entries().len())
            .unwrap_or(0);
        let lead = div()
            .flex()
            .flex_col()
            .gap(px(theme::space::SM))
            .py(px(theme::space::MD))
            .child(
                div()
                    .font_family(theme::display_font())
                    .text_size(px(theme::type_size::TITLE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(palette.text)
                    .child("Your voice stays on this device."),
            )
            .child(paragraph(
                palette,
                "Draft captures audio only while you hold the dictation key. The clip is transcribed on this machine and then discarded.",
            ))
            .child(paragraph(
                palette,
                "The one exception is the model file: it is downloaded once from the internet and checked against a pinned checksum. The app makes no network requests.",
            ))
            .child(paragraph(
                palette,
                "Finished dictations are kept in a local history file so you can search them later. That file is never uploaded or synced.",
            ));

        let clear = self
            .text_button(&app, palette, "button:clear-history", "Clear")
            .id("clear-history")
            .on_click(cx.listener(|this, _, _, cx| this.clear_history(cx)));
        let open = self
            .text_button(&app, palette, "button:open-data", "Open")
            .id("open-data")
            .on_click(cx.listener(|this, _, _, cx| this.open_data_folder(cx)));

        vec![
            lead.into_any_element(),
            setting_row(
                palette,
                "Local history",
                &format!("{history_count} dictations stored in %LOCALAPPDATA%\\Draft4"),
                clear,
            )
            .into_any_element(),
            setting_row(
                palette,
                "Data folder",
                "Settings, models, logs, and history live here.",
                open,
            )
            .into_any_element(),
        ]
    }

    fn advanced_rows(
        &mut self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let app = cx.entity().downgrade();
        let load = match self.load_ms {
            Some(milliseconds) => format!("{:.1} s", milliseconds as f64 / 1000.0),
            None => "Not measured yet".to_owned(),
        };
        let open = self
            .text_button(&app, palette, "button:open-logs", "Open")
            .id("open-logs")
            .on_click(cx.listener(|this, _, _, cx| this.open_logs_folder(cx)));
        let last = match self.last.as_ref() {
            Some(result) => format!(
                "{:.0} ms engine · {:.1} s audio",
                result.engine_ms as f64, result.audio_s
            ),
            None => "No dictation yet".to_owned(),
        };

        vec![
            setting_row(
                palette,
                "Version",
                "Draft for Windows.",
                div()
                    .text_size(px(theme::type_size::SMALL))
                    .text_color(palette.secondary)
                    .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
            )
            .into_any_element(),
            setting_row(
                palette,
                "Model load",
                "Time to load and warm the model at launch.",
                div()
                    .text_size(px(theme::type_size::SMALL))
                    .text_color(palette.secondary)
                    .child(load),
            )
            .into_any_element(),
            setting_row(
                palette,
                "Last dictation",
                "Time for the most recent transcript.",
                div()
                    .text_size(px(theme::type_size::SMALL))
                    .text_color(palette.secondary)
                    .child(last),
            )
            .into_any_element(),
            setting_row(palette, "Logs", "Diagnostics from the last sessions.", open)
                .into_any_element(),
            setting_row(
                palette,
                "Command line",
                "Install or verify the model from a terminal.",
                div()
                    .text_size(px(theme::type_size::META))
                    .text_color(palette.tertiary)
                    .child("draftctl model install"),
            )
            .into_any_element(),
        ]
    }
}

impl Render for DraftApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette_for(window);
        self.palette = palette;
        let (view, opacity) = match self.transition.as_ref() {
            Some(transition) if !transition.swapped => {
                let progress =
                    (transition.started.elapsed().as_secs_f32() / FADE_OUT.as_secs_f32()).min(1.0);
                (transition.from, 1.0 - progress)
            }
            Some(transition) => {
                let progress =
                    (transition.started.elapsed().as_secs_f32() / FADE_IN.as_secs_f32()).min(1.0);
                (transition.to, progress)
            }
            None => (self.view, 1.0),
        };
        let root = div()
            .id("draft-root")
            .size_full()
            .bg(palette.canvas)
            .text_color(palette.text)
            .font_family(theme::text_font());

        match view {
            View::Dictate => root.child(self.dictate_view(&palette, cx).opacity(opacity)),
            View::Settings => root.child(self.settings_view(&palette, cx).opacity(opacity)),
        }
    }
}

fn headline(text: &str, color: Rgba) -> Div {
    div()
        .font_family(theme::display_font())
        .text_size(px(theme::type_size::HEADLINE))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(color)
        .child(text.to_owned())
}

fn secondary(palette: &theme::Palette, text: &str) -> Div {
    div()
        .text_size(px(theme::type_size::BODY))
        .text_color(palette.secondary)
        .child(text.to_owned())
}

fn paragraph(palette: &theme::Palette, text: &str) -> Div {
    div()
        .text_size(px(theme::type_size::BODY))
        .line_height(px(19.0))
        .text_color(palette.secondary)
        .max_w(px(420.0))
        .child(text.to_owned())
}

/// Ease a control's hover state instead of switching it instantly.
fn hoverable(
    app: WeakEntity<DraftApp>,
    key: &'static str,
    element: Stateful<Div>,
) -> Stateful<Div> {
    let app = app.clone();
    element.on_hover(move |hovered, _, cx| {
        let _ = app.update(cx, |this, cx| {
            this.motion.set_hover(key, if *hovered { 1.0 } else { 0.0 });
            cx.notify();
        });
    })
}

/// Give a control a press state, so a click is felt as well as seen.
fn pressable(
    app: WeakEntity<DraftApp>,
    key: &'static str,
    element: Stateful<Div>,
) -> Stateful<Div> {
    let down = app.clone();
    let up = app.clone();
    element
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            let _ = down.update(cx, |this, cx| {
                this.motion.set_press(key, 1.0);
                cx.notify();
            });
        })
        .on_mouse_up_out(gpui::MouseButton::Left, move |_, _, cx| {
            let _ = up.update(cx, |this, cx| {
                this.motion.set_press(key, 0.0);
                cx.notify();
            });
        })
}

fn keycap(palette: &theme::Palette, label: &str) -> Div {
    div()
        .px(px(8.0))
        .py(px(3.0))
        .rounded_md()
        .bg(palette.keycap)
        .border_1()
        .border_color(palette.keycap_border)
        .text_size(px(theme::type_size::BODY))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(palette.text)
        .child(label.to_owned())
}

fn brand_mark(palette: &theme::Palette, size: f32) -> Div {
    let bars = [0.45, 0.8, 1.0, 0.6];
    let width = (size * 0.13).max(1.5);
    let gap = size * 0.12;
    div()
        .w(px(size))
        .h(px(size))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(gap))
        .children(bars.into_iter().map(|factor| {
            div()
                .w(px(width))
                .h(px(factor * size * 0.6))
                .rounded_full()
                .bg(palette.accent)
        }))
}

/// A label and its explanation, with the control on the right.
fn setting_row(
    palette: &theme::Palette,
    title: &str,
    description: &str,
    control: impl IntoElement,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(theme::space::LG))
        .py(px(10.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .flex_1()
                .min_w(px(0.0))
                .child(
                    div()
                        .text_size(px(theme::type_size::BODY))
                        .line_height(px(18.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(palette.text)
                        .child(title.to_owned()),
                )
                .child(
                    div()
                        .text_size(px(theme::type_size::SMALL))
                        .line_height(px(16.0))
                        .text_color(palette.tertiary)
                        .child(description.to_owned()),
                ),
        )
        .child(div().flex_shrink_0().child(control))
}

/// A label, its explanation, and content that needs the full width.
fn setting_block(
    palette: &theme::Palette,
    title: &str,
    description: &str,
    content: impl IntoElement,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(theme::space::SM))
        .py(px(10.0))
        .child(
            div()
                .text_size(px(theme::type_size::BODY))
                .line_height(px(18.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(palette.text)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_size(px(theme::type_size::SMALL))
                .line_height(px(16.0))
                .text_color(palette.tertiary)
                .child(description.to_owned()),
        )
        .child(content)
}

/// A selectable option. Selection is a quiet wash, not another border.
fn pasted_ago(at: Instant) -> String {
    let seconds = at.elapsed().as_secs();
    match seconds {
        0..=4 => "Pasted just now".to_owned(),
        5..=59 => format!("Pasted {seconds} seconds ago"),
        60..=3599 => format!("Pasted {} minutes ago", seconds / 60),
        _ => format!("Pasted {} hours ago", seconds / 3600),
    }
}

/// Open a path in the platform file manager without flashing a console.
fn open_in_file_manager(path: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("explorer")
            .arg(path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
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

/// The newest words of a rolling preview, so the live view keeps up as the
/// speaker continues instead of showing the start of the window.
fn partial_preview(text: &str) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.len() <= 200 {
        text.to_owned()
    } else {
        format!("…{}", chars[chars.len() - 200..].iter().collect::<String>())
    }
}
