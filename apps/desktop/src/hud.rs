//! The click-through dictation HUD.
//!
//! It lives in its own transparent popup window so it can stay above the target
//! application without taking focus. The main window owns the state; this view
//! draws the real microphone history and the newest words that were heard.
//!
//! The meter is the same data as the window's input band, sampled coarser. It
//! rests on a flat line when there is no sound and only rises on real input.

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};

use crate::theme;

/// Sixteen 3 px bars with 3 px gutters fill the meter.
const METER_BARS: usize = 16;
const BAR_WIDTH: f32 = 3.0;
const METER_WIDTH: f32 = 93.0;
const METER_HEIGHT: f32 = 28.0;
/// Newest characters of a rolling preview that fit the detail line.
const PREVIEW_CHARS: usize = 46;

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
        hotkey: &str,
        partial: Option<String>,
        bars: Vec<f32>,
        elapsed: Option<String>,
        palette: theme::Palette,
    ) -> Self {
        Self {
            title: "Listening".to_owned(),
            detail: format!("Release {hotkey} to transcribe"),
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
            detail: "Finishing on this device".to_owned(),
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
        let meter = if listening && !quiet {
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
        } else {
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
        };

        let body = snapshot
            .partial
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| (clip_tail(text, PREVIEW_CHARS), palette.text))
            .unwrap_or_else(|| (snapshot.detail.clone(), palette.secondary));

        // The right edge carries the take length beside the cancel key. While
        // the model works there is no key to press, so the dots take that
        // place and the pill stays balanced.
        let trailing = if listening {
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .w(px(46.0))
                        .flex()
                        .justify_end()
                        .font_family(theme::text_font())
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_size(px(theme::type_size::SMALL))
                        .line_height(px(theme::line::SMALL))
                        .text_color(palette.text)
                        .child(
                            snapshot
                                .elapsed
                                .clone()
                                .unwrap_or_else(|| "0:00.0".to_owned()),
                        ),
                )
                .child(
                    div()
                        .w(px(36.0))
                        .h(px(22.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .bg(palette.raised)
                        .border_1()
                        .border_color(palette.hairline)
                        .font_family(theme::text_font())
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_size(px(10.0))
                        .line_height(px(12.0))
                        .text_color(palette.secondary)
                        .child("ESC"),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .children((0..3).map(|index| {
                    let lit = index == usize::from(snapshot.beat % 3);
                    div().w(px(6.0)).h(px(6.0)).rounded_full().bg(if lit {
                        palette.accent_text
                    } else {
                        palette.track
                    })
                }))
                .into_any_element()
        };

        // The pill tightens when the microphone closes, so the working state
        // never looks like a wide panel with two short lines in it.
        let width = if listening { 540.0 } else { 400.0 };

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
                    .gap(px(14.0))
                    .w(px(width))
                    .h(px(56.0))
                    .pl(px(22.0))
                    .pr(px(14.0))
                    .rounded_full()
                    .bg(palette.surface)
                    .border_1()
                    .border_color(palette.hairline_strong)
                    .shadow(theme::hud_shadow())
                    .child(
                        div()
                            .w(px(10.0))
                            .h(px(10.0))
                            .rounded_full()
                            .bg(if listening {
                                palette.accent
                            } else {
                                palette.tertiary
                            }),
                    )
                    .child(meter)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.0))
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .line_height(px(13.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(if listening {
                                        palette.accent_text
                                    } else {
                                        palette.tertiary
                                    })
                                    .child(snapshot.title.to_uppercase()),
                            )
                            .child(
                                div()
                                    .text_size(px(theme::type_size::SMALL))
                                    .line_height(px(theme::line::SMALL))
                                    .text_color(body.1)
                                    .h(px(17.0))
                                    .overflow_hidden()
                                    .child(body.0),
                            ),
                    )
                    .child(trailing),
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
