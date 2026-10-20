//! The one Draft window: live state, the dictation loop, and the engine
//! hand-off. The model lives on its own thread; this view only polls events.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use draft_audio::Recorder;
use draft_engine::{ComputeBackend, EngineTimings};
use draft_hotkey::{HotkeyConfig, HotkeyEvent, HotkeyListener};
use gpui::{Context, Div, IntoElement, Render, Rgba, Window, div, prelude::*, px};

use crate::theme;
use crate::worker::{self, Job};

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

pub struct DraftApp {
    phase: Phase,
    engine_ready: bool,
    backend: Option<String>,
    device: Option<String>,
    load_ms: Option<u128>,
    _hotkey: Option<HotkeyListener>,
    hotkey_events: Option<Receiver<HotkeyEvent>>,
    hotkey_error: Option<String>,
    hotkey_label: String,
    recorder: Option<Recorder>,
    jobs: Sender<Job>,
    worker_events: Receiver<worker::Event>,
    pending_jobs: usize,
    last: Option<LastResult>,
    error: Option<String>,
    notice: Option<String>,
}

impl DraftApp {
    pub fn new(
        _cx: &mut Context<Self>,
        model: PathBuf,
        backend: ComputeBackend,
        paste: bool,
        hotkey: HotkeyConfig,
    ) -> Self {
        let (jobs, worker_events) = worker::spawn(model, backend, paste);
        let hotkey_label = hotkey.name.to_owned();
        let (listener, hotkey_events, hotkey_error) = match draft_hotkey::start(hotkey) {
            Ok((listener, events)) => (Some(listener), Some(events), None),
            Err(error) => (
                None,
                None,
                Some(format!("The global hotkey is unavailable: {error:#}")),
            ),
        };
        let initial_error = hotkey_error.clone();

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
            hotkey_events,
            hotkey_error,
            hotkey_label,
            recorder: None,
            jobs,
            worker_events,
            pending_jobs: 0,
            last: None,
            error: initial_error,
            notice: None,
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
        if changed {
            cx.notify();
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
            worker::Event::Transcribed {
                text,
                elapsed,
                timings,
                audio_s,
                paste_error,
            } => {
                self.pending_jobs = self.pending_jobs.saturating_sub(1);
                self.last = Some(LastResult {
                    text,
                    engine_ms: elapsed.as_millis(),
                    audio_s,
                    _timings: timings,
                    paste_error,
                });
                self.notice = None;
                self.refresh_phase();
            }
            worker::Event::Empty { elapsed } => {
                self.pending_jobs = self.pending_jobs.saturating_sub(1);
                self.notice = Some(format!(
                    "No speech detected in that clip ({:.0} ms).",
                    elapsed.as_secs_f64() * 1000.0
                ));
                self.refresh_phase();
            }
            worker::Event::JobFailed { error } => {
                self.pending_jobs = self.pending_jobs.saturating_sub(1);
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
        match Recorder::start() {
            Ok(recorder) => {
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
        let load = self
            .load_ms
            .map(|load| format!("loaded {:.1} s", load as f64 / 1000.0))
            .unwrap_or_else(|| "one model stays warm".to_owned());

        let mut root = div()
            .size_full()
            .bg(theme::background())
            .flex()
            .flex_col()
            .p_8()
            .gap_5()
            .child(
                div()
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
                                    .w(px(10.0))
                                    .h(px(10.0))
                                    .rounded_full()
                                    .bg(theme::accent()),
                            )
                            .child(div().text_2xl().text_color(theme::text()).child("Draft")),
                    )
                    .child(
                        div()
                            .id("quit")
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .border_1()
                            .border_color(theme::border())
                            .text_sm()
                            .text_color(theme::text_muted())
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme::text()))
                            .on_click(cx.listener(|_, _, _, cx| cx.quit()))
                            .child("Quit"),
                    ),
            )
            .child(
                div()
                    .bg(theme::surface())
                    .rounded_lg()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().w(px(10.0)).h(px(10.0)).rounded_full().bg(color))
                            .child(div().text_xl().text_color(theme::text()).child(status)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme::text_muted())
                            .child(detail),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(info_row(
                        "Shortcut",
                        self.hotkey_label.clone(),
                        "hold to dictate".to_owned(),
                    ))
                    .child(info_row(
                        "Model",
                        "Whisper large-v3-turbo".to_owned(),
                        "Q8_0".to_owned(),
                    ))
                    .child(info_row("Engine", engine, load)),
            );

        if let Some(last) = &self.last {
            let mut card = div()
                .bg(theme::surface())
                .rounded_lg()
                .p_5()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::text_muted())
                        .child("Last dictation"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::text())
                        .child(preview(&last.text)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(format!(
                            "{:.0} ms engine · {:.1}s audio",
                            last.engine_ms as f64, last.audio_s
                        )),
                );
            if let Some(error) = &last.paste_error {
                card = card.child(
                    div()
                        .text_xs()
                        .text_color(theme::danger())
                        .child(format!("Paste warning: {error}")),
                );
            }
            root = root.child(card);
        }

        if let Some(error) = &self.error {
            root = root.child(
                div()
                    .bg(theme::surface())
                    .rounded_lg()
                    .p_4()
                    .text_xs()
                    .text_color(theme::danger())
                    .child(error.clone()),
            );
        }
        if let Some(notice) = &self.notice {
            root = root.child(
                div()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child(notice.clone()),
            );
        }

        root.child(
            div()
                .mt_auto()
                .text_xs()
                .text_color(theme::text_muted())
                .child("Local audio. Local text. Nothing leaves this machine."),
        )
    }
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

fn info_row(label: &'static str, value: String, detail: String) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .bg(theme::surface())
        .rounded_lg()
        .px_5()
        .py_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_xs().text_color(theme::text_muted()).child(label))
                .child(div().text_sm().text_color(theme::text()).child(value)),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::text_muted())
                .child(detail),
        )
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
