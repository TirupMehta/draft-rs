//! Desktop design tokens.
//!
//! One place for colors and geometry so panes never invent raw values. The
//! palette is Draft's golden-hour look, carried over from the shipping app.

use gpui::{Rgba, rgb};

pub fn background() -> Rgba {
    rgb(0x14100c)
}

pub fn surface() -> Rgba {
    rgb(0x1f1913)
}

pub fn text() -> Rgba {
    rgb(0xf3e7d3)
}

pub fn text_muted() -> Rgba {
    rgb(0xa89b86)
}

pub fn accent() -> Rgba {
    rgb(0xd9a441)
}
