//! The tray icon and its small menu. It keeps Draft reachable after the
//! settings window is hidden.

use anyhow::Result;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuItem, PredefinedMenuItem},
};

pub const SHOW_ID: &str = "draft.show";
pub const PASTE_LAST_ID: &str = "draft.paste-last";
pub const QUIT_ID: &str = "draft.quit";

pub fn build(hotkey: &str) -> Result<TrayIcon> {
    let menu = Menu::new();
    menu.append_items(&[
        &MenuItem::with_id(SHOW_ID, "Show Draft", true, None),
        &MenuItem::with_id(PASTE_LAST_ID, "Paste last dictation", true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(QUIT_ID, "Quit Draft", true, None),
    ])?;

    let icon = Icon::from_rgba(icon_rgba(), 32, 32)?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip(format!("Draft - hold {hotkey} to dictate"))
        .with_icon(icon)
        .build()?;

    Ok(tray)
}

/// A 32 px amber disc with three dark waveform bars. Generated in code so the
/// shell has no asset pipeline yet.
fn icon_rgba() -> Vec<u8> {
    const SIZE: i32 = 32;
    let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - 16.0;
            let dy = y as f32 + 0.5 - 16.0;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 15.0 {
                continue;
            }
            let alpha = ((15.0 - distance).clamp(0.0, 1.0) * 255.0) as u8;
            let index = ((y * SIZE + x) * 4) as usize;
            pixels[index..index + 4].copy_from_slice(&[0xd9, 0xa4, 0x41, alpha]);
        }
    }

    for (center_x, half_height) in [(11_i32, 4_i32), (16, 7), (21, 5)] {
        for y in (16 - half_height)..(16 + half_height) {
            for x in (center_x - 1)..(center_x + 1) {
                let index = ((y * SIZE + x) * 4) as usize;
                pixels[index..index + 4].copy_from_slice(&[0x14, 0x10, 0x0c, 255]);
            }
        }
    }

    pixels
}
