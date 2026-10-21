//! Desktop design tokens.
//!
//! One place for colors and geometry so panes never invent raw values. A near
//! black canvas keeps the floating HUD quiet over any application; the amber
//! accent is the one warm signal in the product.

use gpui::{Rgba, rgb};

pub fn background() -> Rgba {
    rgb(0x0c0c0e)
}

pub fn surface() -> Rgba {
    rgb(0x151519)
}

pub fn elevated() -> Rgba {
    rgb(0x1d1d23)
}

pub fn text() -> Rgba {
    rgb(0xf4f1ea)
}

pub fn text_muted() -> Rgba {
    rgb(0x98938a)
}

pub fn accent() -> Rgba {
    rgb(0xf0b45c)
}

pub fn border() -> Rgba {
    rgb(0x2a2a31)
}

pub fn success() -> Rgba {
    rgb(0x74c69d)
}

pub fn danger() -> Rgba {
    rgb(0xef7d6f)
}
