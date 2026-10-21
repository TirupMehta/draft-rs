//! The click-through dictation HUD.
//!
//! It lives in its own transparent popup window so it can stay above the target
//! application without taking focus. The main window owns the state; this view
//! draws the real microphone level and the newest words that were heard.

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
    pub palette: theme::Palette,
}

impl HudSnapshot {
    pub fn listening(
        hotkey: &str,
        partial: Option<String>,
        level: f32,
        palette: theme::Palette,
    ) -> Self {
        Self {
            title: "Listening".to_owned(),
            detail: format!("Release {hotkey} to transcribe"),
            partial,
            level,
            tone: HudTone::Listening,
            palette,
        }
    }

    pub fn transcribing(palette: theme::Palette) -> Self {
        Self {
            title: "Transcribing".to_owned(),
            detail: "Finishing on this device".to_owned(),
            partial: None,
            level: 0.0,
            tone: HudTone::Working,
            palette,
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

        let palette = snapshot.palette;
        let listening = snapshot.tone == HudTone::Listening;
        let accent = if listening {
            palette.accent
        } else {
            palette.tertiary
        };

        // The level is the real microphone level, so the meter moves only when
        // there is something to hear.
        let phase = self.started.elapsed().as_secs_f32() * 6.0;
        let amplitude = (snapshot.level.sqrt() * 1.15).clamp(0.0, 1.0);
        let bars = (0..12)
            .map(|index| {
                let wave = ((phase + index as f32 * 0.5).sin() * 0.5 + 0.5).powi(2);
                let height = 4.0 + wave * (4.0 + amplitude * 18.0);
                div()
                    .w(px(2.0))
                    .h(px(height))
                    .rounded_full()
                    .bg(if amplitude > 0.02 {
                        accent
                    } else {
                        palette.track
                    })
            })
            .collect::<Vec<_>>();

        let body = snapshot
            .partial
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| (clip_tail(text, 96), palette.text))
            .unwrap_or_else(|| (snapshot.detail.clone(), palette.secondary));

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .w(px(560.0))
                    .px(px(14.0))
                    .py(px(10.0))
                    .rounded_xl()
                    .bg(palette.canvas)
                    .border_1()
                    .border_color(palette.hairline)
                    .shadow(theme::hud_shadow())
                    .child(
                        div()
                            .w(px(40.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(2.0))
                            .children(bars),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.0))
                            .gap(px(2.0))
                            .child(
                                div()
                                    .font_family(theme::display_font())
                                    .text_size(px(13.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(if listening {
                                        palette.text
                                    } else {
                                        palette.secondary
                                    })
                                    .child(snapshot.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(body.1)
                                    .h(px(16.0))
                                    .overflow_hidden()
                                    .child(body.0),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py(px(3.0))
                            .rounded_md()
                            .bg(palette.raised)
                            .text_size(px(11.0))
                            .text_color(palette.tertiary)
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
