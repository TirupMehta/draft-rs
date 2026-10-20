//! Draft's desktop shell.
//!
//! M0 draws the first native window. The dictation loop, tray, and HUD land in
//! the following milestones; see `docs/ROADMAP.md`.

mod theme;

use gpui::prelude::*;
use gpui::{
    App, Application, Bounds, Context, IntoElement, Render, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, px, size,
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
            .child(
                div()
                    .bg(theme::surface())
                    .rounded_lg()
                    .p_8()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(div().text_2xl().text_color(theme::text()).child("Draft"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme::text_muted())
                            .child("Local dictation for every device"),
                    )
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(theme::accent()),
                    ),
            )
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
                titlebar: Some(TitlebarOptions {
                    title: Some("Draft".into()),
                    ..Default::default()
                }),
                app_id: Some("in.tirup.draft".into()),
                ..Default::default()
            },
            |_, cx| cx.new(|_| DraftWindow),
        )
        .expect("could not open the Draft window");
    });

    Ok(())
}
