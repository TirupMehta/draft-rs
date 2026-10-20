//! The small, click-through dictation HUD.
//!
//! It lives in its own transparent popup window so it can stay above the
//! target application without taking focus. The main window owns the state;
//! this view only draws the latest snapshot.

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};

use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudTone {
    Listening,
    Working,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HudSnapshot {
    pub title: String,
    pub detail: String,
    pub tone: HudTone,
}

impl HudSnapshot {
    pub fn listening(hotkey: &str) -> Self {
        Self {
            title: "Listening".to_owned(),
            detail: format!("Release {hotkey} to transcribe"),
            tone: HudTone::Listening,
        }
    }

    pub fn transcribing() -> Self {
        Self {
            title: "Transcribing".to_owned(),
            detail: "Everything stays on this machine".to_owned(),
            tone: HudTone::Working,
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

        let color = match snapshot.tone {
            HudTone::Listening | HudTone::Working => theme::accent(),
        };

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
                    .gap_3()
                    .px_5()
                    .py_3()
                    .rounded_full()
                    .bg(theme::surface())
                    .border_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .w(px(22.0))
                            .h(px(22.0))
                            .rounded_full()
                            .bg(color)
                            .child(
                                div()
                                    .w(px(6.0))
                                    .h(px(6.0))
                                    .rounded_full()
                                    .bg(theme::background()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme::text())
                                    .child(snapshot.title.clone()),
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
                            .ml_2()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child(match snapshot.tone {
                                HudTone::Listening => "ESC",
                                HudTone::Working => "LOCAL",
                            }),
                    ),
            )
    }
}
