//! Draft's desktop shell.
//!
//! The window is the control surface; dictation itself works while any other
//! app has focus. The engine thread loads and warms one model at launch.

mod app;
mod hud;
mod theme;
mod tray;
mod window_control;
mod worker;

use std::path::PathBuf;
use std::time::Duration;

use draft_engine::ComputeBackend;
use draft_hotkey::HotkeyConfig;
use gpui::prelude::*;
use gpui::{
    App, Application, Bounds, Timer, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, point, px, size,
};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let options = parse_options();

    Application::new().run(move |cx: &mut App| {
        let hud = cx.new(|_| hud::DraftHud::new());
        let app = cx.new(|cx| {
            app::DraftApp::new(
                cx,
                options.model.clone(),
                options.backend,
                options.paste,
                options.hotkey,
                hud.clone(),
            )
        });

        let bounds = Bounds::centered(None, size(px(760.0), px(720.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Draft".into()),
                        ..Default::default()
                    }),
                    app_id: Some("in.tirup.draft".into()),
                    ..Default::default()
                },
                |_, _| app.clone(),
            )
            .expect("could not open the Draft window");

        let main_handle = cx.active_window();
        let tray_available = app.read(cx).has_tray();
        let main_hwnd = window
            .update(cx, |_, window, cx| {
                let hwnd = window_control::hwnd(window);
                window.on_window_should_close(cx, move |window, _| {
                    if tray_available {
                        !window_control::hide(window)
                    } else {
                        true
                    }
                });
                window.activate_window();
                hwnd
            })
            .ok()
            .flatten();

        let hud_size = size(px(440.0), px(96.0));
        let screen = cx
            .primary_display()
            .or_else(|| cx.displays().into_iter().next())
            .expect("Draft needs a display")
            .bounds();
        let hud_bounds = Bounds {
            origin: point(
                screen.center().x - hud_size.center().x,
                screen.origin.y + screen.size.height - hud_size.height - px(92.0),
            ),
            size: hud_size,
        };
        let hud_window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(hud_bounds)),
                    titlebar: None,
                    focus: false,
                    show: true,
                    kind: WindowKind::PopUp,
                    is_movable: false,
                    is_resizable: false,
                    is_minimizable: false,
                    window_background: WindowBackgroundAppearance::Transparent,
                    app_id: Some("in.tirup.draft.hud".into()),
                    ..Default::default()
                },
                move |_, _| hud.clone(),
            )
            .expect("could not open the Draft HUD");
        let hud_hwnd = hud_window
            .update(cx, |_, window, cx| {
                // The HUD has no close affordance and must outlive every
                // dictation. A stray WM_CLOSE (for example from a launcher
                // that picks the first top-level window) is ignored.
                window.on_window_should_close(cx, |_, _| false);
                window_control::hwnd(window)
            })
            .ok()
            .flatten();
        if let Some(hwnd) = hud_hwnd {
            window_control::configure_hud(hwnd);
        }

        app.update(cx, |app, _| {
            app.attach_windows(main_handle, main_hwnd, hud_hwnd);
        });

        let poll_app = app.clone();
        cx.spawn(async move |cx| {
            loop {
                Timer::after(Duration::from_millis(16)).await;
                if poll_app.update(cx, |app, cx| app.poll(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    });

    Ok(())
}

struct Options {
    model: PathBuf,
    backend: ComputeBackend,
    paste: bool,
    hotkey: HotkeyConfig,
}

fn parse_options() -> Options {
    let mut model = default_model_path();
    let mut explicit_model = false;
    let mut backend = ComputeBackend::Auto;
    let mut paste = true;
    let mut hotkey = HotkeyConfig::default();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--model" | "-m" => {
                let Some(path) = args.next() else {
                    eprintln!("--model needs a path");
                    std::process::exit(2);
                };
                model = PathBuf::from(path);
                explicit_model = true;
            }
            "--backend" => {
                let Some(value) = args.next() else {
                    eprintln!("--backend needs a value");
                    std::process::exit(2);
                };
                backend = match value.as_str() {
                    "auto" => ComputeBackend::Auto,
                    "vulkan" => ComputeBackend::Vulkan,
                    "metal" => ComputeBackend::Metal,
                    "cuda" => ComputeBackend::Cuda,
                    "cpu" => ComputeBackend::Cpu,
                    other => {
                        eprintln!("unknown backend: {other}");
                        std::process::exit(2);
                    }
                };
            }
            "--no-paste" => paste = false,
            "--hotkey" => {
                let Some(value) = args.next() else {
                    eprintln!("--hotkey needs a value");
                    std::process::exit(2);
                };
                hotkey = match HotkeyConfig::named(&value) {
                    Some(hotkey) => hotkey,
                    None => {
                        eprintln!("unknown hotkey: {value}");
                        std::process::exit(2);
                    }
                };
            }
            "--help" | "-h" => {
                println!("{}", help());
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}\n\n{}", help());
                std::process::exit(2);
            }
        }
    }

    if !explicit_model && !model.exists() && cfg!(all(debug_assertions, target_os = "windows")) {
        // Development convenience: the benchmark model lives outside the app
        // data directory. Release builds never look here.
        let dev = PathBuf::from(r"C:\all\models\whisper-large-v3-turbo-Q8_0.gguf");
        if dev.exists() {
            model = dev;
        }
    }

    Options {
        model,
        backend,
        paste,
        hotkey,
    }
}

fn default_model_path() -> PathBuf {
    if let Ok(path) = std::env::var("DRAFT_MODEL_PATH") {
        return PathBuf::from(path);
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Draft")
        .join("models")
        .join("whisper-large-v3-turbo-Q8_0.gguf")
}

fn help() -> &'static str {
    "Draft desktop

Usage:
  draft [--model PATH] [--backend auto|vulkan|cpu] [--hotkey NAME] [--no-paste]

The default model path is %LOCALAPPDATA%\\Draft\\models\\
whisper-large-v3-turbo-Q8_0.gguf. Set DRAFT_MODEL_PATH or pass --model to
override it. Hotkeys: right-ctrl (default), right-alt, f8, f9, pause.
Hold the hotkey to dictate; release to paste at the cursor."
}
