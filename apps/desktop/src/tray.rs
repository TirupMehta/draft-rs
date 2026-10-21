//! The tray icon and its small menu. It keeps Draft reachable after the
//! settings window is hidden.

use anyhow::Result;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuItem, PredefinedMenuItem},
};

pub const SHOW_ID: &str = "draft.show";
pub const HIDE_ID: &str = "draft.hide";
pub const PASTE_LAST_ID: &str = "draft.paste-last";
pub const QUIT_ID: &str = "draft.quit";

pub fn build(hotkey: &str) -> Result<TrayIcon> {
    let menu = Menu::new();
    menu.append_items(&[
        &MenuItem::with_id(SHOW_ID, "Show Draft", true, None),
        &MenuItem::with_id(HIDE_ID, "Hide Draft", true, None),
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

    // A dark rounded tile keeps the tray icon consistent with the app's brand
    // mark. The amber bars are the only bright pixels, so the icon reads
    // clearly on both light and dark taskbars.
    for y in 0..SIZE {
        for x in 0..SIZE {
            let radius = 7.0;
            let inside_x = (x as f32).clamp(radius, (SIZE as f32) - 1.0 - radius);
            let inside_y = (y as f32).clamp(radius, (SIZE as f32) - 1.0 - radius);
            let dx = x as f32 + 0.5 - (inside_x + 0.5);
            let dy = y as f32 + 0.5 - (inside_y + 0.5);
            if (dx * dx + dy * dy).sqrt() > radius {
                continue;
            }
            let index = ((y * SIZE + x) * 4) as usize;
            pixels[index..index + 4].copy_from_slice(&[0x10, 0x10, 0x14, 255]);
        }
    }

    for (center_x, half_height) in [(8_i32, 5_i32), (13, 8), (18, 11), (23, 8), (28, 5)] {
        for y in (16 - half_height)..(16 + half_height) {
            for x in (center_x - 1)..(center_x + 1) {
                let index = ((y * SIZE + x) * 4) as usize;
                pixels[index..index + 4].copy_from_slice(&[0xf0, 0xb4, 0x5c, 255]);
            }
        }
    }

    pixels
}
