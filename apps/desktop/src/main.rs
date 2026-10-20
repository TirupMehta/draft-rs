//! Draft's desktop shell.
//!
//! The window is the control surface; dictation itself works while any other
//! app has focus. The engine thread loads and warms one model at launch.

mod app;
mod theme;
mod worker;

use std::path::PathBuf;
use std::time::Duration;

use draft_engine::ComputeBackend;
use draft_hotkey::HotkeyConfig;
use gpui::prelude::*;
use gpui::{
    App, Application, Bounds, Timer, TitlebarOptions, WindowBounds, WindowOptions, px, size,
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
        let app = cx.new(|cx| {
            app::DraftApp::new(
                cx,
                options.model.clone(),
                options.backend,
                options.paste,
                options.hotkey,
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

        window
            .update(cx, |_, window, cx| {
                window.on_window_should_close(cx, |_, cx| {
                    cx.quit();
                    true
                });
                window.activate_window();
            })
            .ok();
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
