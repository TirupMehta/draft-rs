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

/// The Draft mark on a dark tile: three rising bars and the half-disc D.
/// Generated in code so the shell has no asset pipeline.
fn icon_rgba() -> Vec<u8> {
    const SIZE: i32 = 32;
    let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];

    let put = |pixels: &mut [u8], x: i32, y: i32, color: [u8; 4]| {
        if !(0..SIZE).contains(&x) || !(0..SIZE).contains(&y) {
            return;
        }
        let index = ((y * SIZE + x) * 4) as usize;
        pixels[index..index + 4].copy_from_slice(&color);
    };

    // A dark rounded tile keeps the amber mark legible on either taskbar.
    for y in 0..SIZE {
        for x in 0..SIZE {
            let radius = 7.0;
            let inside_x = (x as f32).clamp(radius, (SIZE as f32) - 1.0 - radius);
            let inside_y = (y as f32).clamp(radius, (SIZE as f32) - 1.0 - radius);
            let dx = x as f32 + 0.5 - (inside_x + 0.5);
            let dy = y as f32 + 0.5 - (inside_y + 0.5);
            if (dx * dx + dy * dy).sqrt() <= radius {
                put(&mut pixels, x, y, [0x10, 0x10, 0x14, 255]);
            }
        }
    }

    let mark = [0xf7, 0xf4, 0xee, 255];
    for (left, half_height) in [(4_i32, 3_i32), (9, 5), (14, 10)] {
        for y in (16 - half_height)..(16 + half_height) {
            for x in left..(left + 3) {
                put(&mut pixels, x, y, mark);
            }
        }
    }

    // The right half of a circle: flat left edge, round right edge.
    let (center_x, center_y, radius) = (19_i32, 16_i32, 10_i32);
    for y in (center_y - radius)..=(center_y + radius) {
        for x in center_x..=(center_x + radius) {
            let dx = x - center_x;
            let dy = y - center_y;
            if dx * dx + dy * dy <= radius * radius {
                put(&mut pixels, x, y, mark);
            }
        }
    }

    pixels
}
