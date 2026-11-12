//! The click-through dictation HUD.
//!
//! It lives in its own transparent popup window so it can stay above the target
//! application without taking focus. The main window owns the state; this view
//! draws the real microphone history, the take length, and, when live preview is
//! on, the newest words that were heard.
//!
//! The meter is the same data as the window's input band, sampled coarser. It
//! rests on a row of dots when there is no sound and only rises on real input.
//! Nothing else is drawn: no title, no hint, no key cap.

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};

use crate::theme;

/// Sixteen 3 px bars with 3 px gutters fill the meter.
const METER_BARS: usize = 16;
const BAR_WIDTH: f32 = 3.0;
const METER_WIDTH: f32 = 93.0;
const METER_HEIGHT: f32 = 24.0;
/// Newest characters of a rolling preview that fit the preview pill.
const PREVIEW_CHARS: usize = 46;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudTone {
    Listening,
    Working,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HudSnapshot {
    /// The label shown while the model works.
    pub title: String,
    /// The newest preview pass, when there is one. Never the final text.
    pub partial: Option<String>,
    /// Amplitude history, oldest first, 0..1. Empty when the microphone is off.
    pub bars: Vec<f32>,
    /// The length of the take in progress, when there is one.
    pub elapsed: Option<String>,
    pub tone: HudTone,
    /// Advances while the model works so the activity dots move.
    pub beat: u8,
    pub palette: theme::Palette,
}

impl HudSnapshot {
    pub fn listening(
        partial: Option<String>,
        bars: Vec<f32>,
        elapsed: Option<String>,
        palette: theme::Palette,
    ) -> Self {
        Self {
            title: "Listening".to_owned(),
            partial,
            bars,
            elapsed,
            tone: HudTone::Listening,
            beat: 0,
            palette,
        }
    }

    pub fn transcribing(beat: u8, palette: theme::Palette) -> Self {
        Self {
            title: "Transcribing".to_owned(),
            partial: None,
            bars: Vec::new(),
            elapsed: None,
            tone: HudTone::Working,
            beat,
            palette,
        }
    }
}

pub struct DraftHud {
    snapshot: Option<HudSnapshot>,
}

impl DraftHud {
    pub fn new() -> Self {
        Self { snapshot: None }
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

        // A quiet take shows a resting row of dots. Bars only appear when the
        // meter has something real to show, so the HUD never animates on
        // silence. Bars grow from the centre line so the meter reads as a
        // waveform rather than a bar chart.
        let quiet = snapshot.bars.iter().all(|bar| *bar < 0.04);
        let meter = if quiet {
            div()
                .w(px(METER_WIDTH))
                .h(px(METER_HEIGHT))
                .flex()
                .items_center()
                .justify_between()
                .children((0..METER_BARS).map(|_| {
                    div()
                        .w(px(BAR_WIDTH))
                        .h(px(BAR_WIDTH))
                        .rounded_full()
                        .bg(palette.track)
                }))
                .into_any_element()
        } else {
            div()
                .w(px(METER_WIDTH))
                .h(px(METER_HEIGHT))
                .flex()
                .items_center()
                .justify_between()
                .children(snapshot.bars.iter().map(|bar| {
                    let weight = bar.clamp(0.0, 1.0);
                    div()
                        .w(px(BAR_WIDTH))
                        .h(px(4.0 + weight * (METER_HEIGHT - 4.0)))
                        .rounded_full()
                        .bg(theme::mix(palette.track, palette.accent, weight.powf(0.6)))
                }))
                .into_any_element()
        };

        let preview = snapshot
            .partial
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| clip_tail(text, PREVIEW_CHARS));

        let timer = div()
            .w(px(46.0))
            .flex()
            .justify_end()
            .font_family(theme::text_font())
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_size(px(theme::type_size::SMALL))
            .line_height(px(theme::line::SMALL))
            .text_color(palette.secondary)
            .child(
                snapshot
                    .elapsed
                    .clone()
                    .unwrap_or_else(|| "0:00.0".to_owned()),
            );

        // Listening: a status dot, the meter, the take length. The words only
        // appear, and the pill only widens, when live preview has something
        // to say. Working: three quiet dots and the one word for what is
        // happening.
        let width = match (listening, preview.is_some()) {
            (true, true) => 420.0,
            (true, false) => 214.0,
            (false, _) => 156.0,
        };

        let content = if listening {
            let mut row = div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .w(px(8.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(palette.accent),
                )
                .child(meter);
            if let Some(text) = preview {
                row = row.child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .h(px(17.0))
                        .overflow_hidden()
                        .text_size(px(theme::type_size::SMALL))
                        .line_height(px(theme::line::SMALL))
                        .text_color(palette.text)
                        .child(text),
                );
            }
            row.child(timer).into_any_element()
        } else {
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .children((0..3).map(|index| {
                            let lit = index == usize::from(snapshot.beat % 3);
                            div().w(px(6.0)).h(px(6.0)).rounded_full().bg(if lit {
                                palette.accent
                            } else {
                                palette.track
                            })
                        })),
                )
                .child(
                    div()
                        .text_size(px(theme::type_size::SMALL))
                        .line_height(px(theme::line::SMALL))
                        .text_color(palette.secondary)
                        .child(snapshot.title.clone()),
                )
                .into_any_element()
        };

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(6.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(width))
                    .h(px(44.0))
                    .px(px(18.0))
                    .rounded_full()
                    .bg(palette.surface)
                    .border_1()
                    .border_color(palette.hairline_strong)
                    .child(content),
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
