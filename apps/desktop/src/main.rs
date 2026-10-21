#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Draft's desktop shell.
//!
//! The window is the control surface; dictation itself works while any other
//! app has focus. The engine thread loads and warms one model at launch.

mod app;
mod hud;
mod motion;
mod startup;
mod theme;
mod tray;
mod window_control;
mod worker;

use std::path::PathBuf;
use std::time::Duration;

use draft_core::settings::Settings;
use draft_engine::ComputeBackend;
use draft_hotkey::HotkeyConfig;
use gpui::prelude::*;
use gpui::{
    App, Application, Bounds, Timer, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, point, px, size,
};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let _log_guard = init_logging();

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
                options.microphone.clone(),
                options.settings.clone(),
                options.settings_path.clone(),
                hud.clone(),
            )
        });

        let bounds = Bounds::centered(None, size(px(560.0), px(240.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Draft".into()),
                        // Draft draws its own title bar so the window reads as
                        // one surface instead of a system frame stacked on
                        // top of an application header.
                        appears_transparent: true,
                        traffic_light_position: None,
                    }),
                    app_id: Some("in.tirup.draft".into()),
                    show: !options.background,
                    ..Default::default()
                },
                |_, _| app.clone(),
            )
            .expect("could not open the Draft window");

        let main_handle = window.into();
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
                if !options.background {
                    window.activate_window();
                }
                hwnd
            })
            .ok()
            .flatten();

        if let Some(hwnd) = main_hwnd {
            // The window frame belongs to Windows. When the person picks a
            // theme, the frame has to follow it.
            match theme::Appearance::from_setting(&options.settings.appearance) {
                theme::Appearance::Light => window_control::set_title_bar_dark(hwnd, false),
                theme::Appearance::Dark => window_control::set_title_bar_dark(hwnd, true),
                theme::Appearance::System => {}
            }
        }

        let hud_size = size(px(600.0), px(84.0));
        let screen = cx
            .primary_display()
            .or_else(|| cx.displays().into_iter().next())
            .expect("Draft needs a display")
            .bounds();
        let hud_bounds = Bounds {
            origin: point(
                screen.center().x - hud_size.center().x,
                screen.origin.y + screen.size.height - hud_size.height - px(64.0),
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
            app.attach_windows(Some(main_handle), main_hwnd, hud_hwnd);
        });

        let poll_app = app.clone();
        cx.spawn(async move |cx| {
            loop {
                // Keep the event poll fast so a hotkey release is not held
                // behind a repaint budget. The app throttles the expensive
                // parts separately: the HUD to 15 fps, the visible window to
                // 30 fps, and a hidden window not at all.
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

/// Keep a diagnostic log next to the settings file. Release builds have no
/// console window, so this is the only place startup and capture problems can
/// be read afterwards.
fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    if cfg!(debug_assertions) {
        tracing_subscriber::fmt().with_env_filter(filter).init();
        return None;
    }

    let dir = draft_core::paths::data_dir().join("logs");
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("could not create the log folder: {error}");
        tracing_subscriber::fmt().with_env_filter(filter).init();
        return None;
    }
    let appender = tracing_appender::rolling::daily(&dir, "draft.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(writer)
        .init();
    Some(guard)
}

struct Options {
    model: PathBuf,
    backend: ComputeBackend,
    paste: bool,
    hotkey: HotkeyConfig,
    microphone: Option<String>,
    settings: Settings,
    settings_path: PathBuf,
    /// Sign-in launches pass this flag so Draft waits in the tray instead of
    /// opening its settings window on top of the desktop.
    background: bool,
}

fn parse_options() -> Options {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let settings_path = cli_settings_path(&args)
        .or_else(|| std::env::var_os("DRAFT_SETTINGS_PATH").map(PathBuf::from))
        .unwrap_or_else(Settings::default_path);
    let settings = Settings::load_or_default(&settings_path);

    let mut model = default_model_path();
    let mut explicit_model = std::env::var_os("DRAFT_MODEL_PATH").is_some();
    if !explicit_model {
        if let Some(saved) = settings.model.clone() {
            model = saved;
            explicit_model = true;
        }
    }
    let mut backend = ComputeBackend::Auto;
    let mut paste = settings.paste;
    let mut hotkey = HotkeyConfig::named(&settings.hotkey).unwrap_or_else(|| {
        tracing::warn!(
            hotkey = %settings.hotkey,
            "unknown saved hotkey; using the default"
        );
        HotkeyConfig::default()
    });
    let mut microphone = settings.microphone.clone();
    let mut background = false;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--settings" => {
                if args.next().is_none() {
                    eprintln!("--settings needs a path");
                    std::process::exit(2);
                }
            }
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
            "--background" => background = true,
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
            "--microphone" => {
                let Some(value) = args.next() else {
                    eprintln!("--microphone needs a device name");
                    std::process::exit(2);
                };
                microphone = if value == "default" {
                    None
                } else {
                    Some(value)
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
        // Development convenience: benchmark artifacts live outside the app
        // data directory. Release builds never look here.
        for candidate in [
            r"C:\all\models\whisper-large-v3-turbo-F16.gguf",
            r"C:\all\models\whisper-large-v3-turbo-Q8_0.gguf",
        ] {
            let dev = PathBuf::from(candidate);
            if dev.exists() {
                model = dev;
                break;
            }
        }
    }

    Options {
        model,
        backend,
        paste,
        hotkey,
        microphone,
        settings,
        settings_path,
        background,
    }
}

fn cli_settings_path(args: &[String]) -> Option<PathBuf> {
    args.iter()
        .position(|argument| argument == "--settings")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from)
}

fn default_model_path() -> PathBuf {
    if let Ok(path) = std::env::var("DRAFT_MODEL_PATH") {
        return PathBuf::from(path);
    }
    draft_core::paths::default_model_path()
}

fn help() -> &'static str {
    "Draft desktop

Usage:
  draft [--model PATH] [--backend auto|vulkan|metal|cuda|cpu] [--hotkey NAME]
        [--microphone NAME|default] [--no-paste] [--background] [--settings PATH]

Settings live at %LOCALAPPDATA%\\Draft4\\settings.json and change from the
Preferences card. CLI flags override them for this launch. Set
DRAFT_SETTINGS_PATH to use a different settings file. The default model path is
%LOCALAPPDATA%\\Draft4\\models\\whisper-large-v3-turbo-F16.gguf; run
`draftctl model install` to download and verify it, or set DRAFT_MODEL_PATH or
pass --model. Hotkeys: right-ctrl (default),
right-alt, f8, f9, pause, scroll-lock. Hold the hotkey to dictate; release to
paste at the cursor."
}
