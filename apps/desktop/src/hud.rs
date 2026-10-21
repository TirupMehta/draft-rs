//! The click-through dictation HUD.
//!
//! It lives in its own transparent popup window so it can stay above the target
//! application without taking focus. The main window owns the state; this view
//! draws a live level meter and the latest preview of what was heard.

use std::time::Instant;

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};

use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudTone {
    Listening,
    Working,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HudSnapshot {
    pub title: String,
    pub detail: String,
    /// The newest preview pass, when there is one. Never the final text.
    pub partial: Option<String>,
    pub level: f32,
    pub tone: HudTone,
}

impl HudSnapshot {
    pub fn listening(hotkey: &str, partial: Option<String>, level: f32) -> Self {
        Self {
            title: "Listening".to_owned(),
            detail: format!("Release {hotkey} to transcribe · Esc cancels"),
            partial,
            level,
            tone: HudTone::Listening,
        }
    }

    pub fn transcribing() -> Self {
        Self {
            title: "Transcribing".to_owned(),
            detail: "Finishing on this device".to_owned(),
            partial: None,
            level: 0.0,
            tone: HudTone::Working,
        }
    }
}

pub struct DraftHud {
    snapshot: Option<HudSnapshot>,
    started: Instant,
}

impl DraftHud {
    pub fn new() -> Self {
        Self {
            snapshot: None,
            started: Instant::now(),
        }
    }

    pub fn set(&mut self, snapshot: Option<HudSnapshot>, cx: &mut Context<Self>) {
        if self.snapshot == snapshot {
            return;
        }
        self.snapshot = snapshot;
        cx.notify();
    }
}

impl Render for DraftHud {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let Some(snapshot) = &self.snapshot else {
            return div().size_full();
        };

        let listening = snapshot.tone == HudTone::Listening;
        let accent = if listening {
            theme::accent()
        } else {
            theme::text_muted()
        };
        let phase = self.started.elapsed().as_secs_f32() * 7.0;
        let amplitude = (snapshot.level.sqrt() * 1.15).clamp(0.0, 1.0);
        let bars = (0..14)
            .map(|index| {
                let wave = ((phase + index as f32 * 0.62).sin() * 0.5 + 0.5).powi(2);
                let height = 5.0 + wave * (6.0 + amplitude * 30.0);
                div().w(px(3.0)).h(px(height)).rounded_full().bg(accent)
            })
            .collect::<Vec<_>>();

        let body = snapshot
            .partial
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| (clip_tail(text, 100), theme::text(), 13.0))
            .unwrap_or_else(|| {
                let text = if listening {
                    "Speak now."
                } else {
                    "Finishing your words…"
                };
                (text.to_owned(), theme::text_muted(), 13.0)
            });

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .w_full()
                    .px_5()
                    .py_4()
                    .rounded_lg()
                    .bg(theme::surface())
                    .border_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_1()
                            .w(px(96.0))
                            .h(px(40.0))
                            .children(bars),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme::text())
                                            .child(snapshot.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py(px(1.0))
                                            .rounded_full()
                                            .bg(theme::background())
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .child("LOCAL"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(body.2))
                                    .text_color(body.1)
                                    .h(px(34.0))
                                    .overflow_hidden()
                                    .child(body.0),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .child(snapshot.detail.clone()),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_lg()
                            .bg(theme::background())
                            .border_1()
                            .border_color(theme::border())
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child(if listening { "ESC" } else { "…" }),
                    ),
            )
    }
}

/// Keep the newest words. While someone is still talking, the end of the
/// rolling preview is what matters, not the start of the window.
fn clip_tail(text: &str, max: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars = normalized.chars().collect::<Vec<_>>();
    if chars.len() <= max {
        normalized
    } else {
        format!("…{}", chars[chars.len() - max..].iter().collect::<String>())
    }
}
