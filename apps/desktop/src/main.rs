//! Draft's desktop shell.
//!
//! M0 draws the first native window. The dictation loop, tray, and HUD land in
//! the following milestones; see `docs/ROADMAP.md`.

mod theme;

use gpui::prelude::*;
use gpui::{
    App, Application, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions,
    div, px, size,
};

struct DraftWindow;

impl Render for DraftWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(theme::background())
            .flex()
            .items_center()
            .justify_center()
            .text_2xl()
            .text_color(theme::text())
            .child("Draft")
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(920.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| DraftWindow),
        )
        .expect("could not open the Draft window");
    });

    Ok(())
}
