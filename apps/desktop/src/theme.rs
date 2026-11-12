//! The Draft design system.
//!
//! One instrument, two finishes. Every color, size, and line the interface
//! uses comes from here, so the product reads as a single object instead of a
//! collection of decorated panels.
//!
//! The rules this file encodes:
//!
//! * One accent. Amber marks live state: recording, selection, focus.
//! * Hierarchy comes from type and space. Containers are the exception.
//! * Light is the default: warm paper, deep amber, quiet hairlines.
//! * Dark is the designed second theme, not an inversion: warm graphite,
//!   lifted surfaces, the same structure and the same restraint.

use gpui::{Rgba, rgb, rgba};

/// How the interface chooses its palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Appearance {
    pub fn from_setting(value: &str) -> Self {
        match value {
            "light" => Appearance::Light,
            "dark" => Appearance::Dark,
            _ => Appearance::System,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Appearance::System => "System",
            Appearance::Light => "Light",
            Appearance::Dark => "Dark",
        }
    }

    pub fn all() -> [Appearance; 3] {
        [Appearance::System, Appearance::Light, Appearance::Dark]
    }

    pub fn index(self) -> usize {
        match self {
            Appearance::System => 0,
            Appearance::Light => 1,
            Appearance::Dark => 2,
        }
    }
}

/// The colors one theme uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// The window background. The floor of the instrument.
    pub canvas: Rgba,
    /// A quiet lifted plane: the settings rail, result blocks, metric strips.
    pub surface: Rgba,
    /// Inputs, switch tracks, and segmented tracks.
    pub raised: Rgba,
    /// The hover state of a raised surface.
    pub raised_hover: Rgba,
    /// A selected or pressed plane above `raised`.
    pub elevated: Rgba,
    /// Meter baselines and empty tracks.
    pub track: Rgba,
    /// Hairlines between rows.
    pub hairline: Rgba,
    /// A line that has to carry weight: the HUD frame, theme dots.
    pub hairline_strong: Rgba,
    /// Primary text.
    pub text: Rgba,
    /// Supporting text.
    pub secondary: Rgba,
    /// Metadata: paths, versions, timestamps.
    pub tertiary: Rgba,
    /// The single accent.
    pub accent: Rgba,
    /// A quiet accent wash.
    pub accent_soft: Rgba,
    /// The accent as readable text.
    pub accent_text: Rgba,
    pub danger: Rgba,
    pub danger_soft: Rgba,
    /// A keyboard key or inline code block.
    pub keycap: Rgba,
    /// A translucent edge under physical keys. Not a box shadow.
    pub shade: Rgba,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            canvas: rgb(0x0f0f12),
            surface: rgb(0x151519),
            raised: rgb(0x1c1c22),
            raised_hover: rgb(0x25252d),
            elevated: rgb(0x2b2b34),
            track: rgb(0x34343e),
            hairline: rgba(0xfff4e214),
            hairline_strong: rgba(0xfff4e228),
            text: rgb(0xf7f4ee),
            secondary: rgb(0xbab6ae),
            tertiary: rgb(0x918d85),
            accent: rgb(0xf2b45f),
            accent_soft: rgba(0xf2b45f24),
            accent_text: rgb(0xf7c77f),
            danger: rgb(0xec7062),
            danger_soft: rgba(0xec706224),
            keycap: rgb(0x202027),
            shade: rgba(0x00000066),
        }
    }

    pub fn light() -> Self {
        Self {
            canvas: rgb(0xf4f1ea),
            surface: rgb(0xfcfaf5),
            raised: rgb(0xece8df),
            raised_hover: rgb(0xe4dfd4),
            elevated: rgb(0xffffff),
            track: rgb(0xdcd6cb),
            hairline: rgba(0x28221817),
            hairline_strong: rgba(0x2822182e),
            text: rgb(0x1b1915),
            secondary: rgb(0x575249),
            tertiary: rgb(0x746f66),
            accent: rgb(0xa3610c),
            accent_soft: rgba(0xa3610c1a),
            accent_text: rgb(0x8a5109),
            danger: rgb(0xb23a2e),
            danger_soft: rgba(0xb23a2e1a),
            keycap: rgb(0xfffdf8),
            shade: rgba(0x0000001f),
        }
    }
}

/// Spacing. The interface uses four, eight, twelve, sixteen, twenty-four,
/// and thirty-two, and nothing in between.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// A fully transparent color, used where a control is selected by fill only.
pub fn transparent() -> Rgba {
    rgba(0x00000000)
}

pub fn white() -> Rgba {
    rgb(0xffffff)
}

/// Blend two colors. Controls use it to fade between their resting and
/// hovered states instead of jumping.
///
/// The blend is premultiplied by alpha. `transparent()` is black at zero alpha,
/// so a straight per-channel blend from it passed through a dark, translucent
/// black on its way to the target: every hover and press flashed muddy. In
/// premultiplied space a fade from transparent keeps the target's own color
/// and only gains opacity. Between two opaque colors the result is unchanged.
pub fn mix(a: Rgba, b: Rgba, amount: f32) -> Rgba {
    let t = amount.clamp(0.0, 1.0);
    let alpha = a.a + (b.a - a.a) * t;
    if alpha <= f32::EPSILON {
        return Rgba {
            r: b.r,
            g: b.g,
            b: b.b,
            a: 0.0,
        };
    }
    let channel = |from: f32, to: f32| (from * a.a * (1.0 - t) + to * b.a * t) / alpha;
    Rgba {
        r: channel(a.r, b.r),
        g: channel(a.g, b.g),
        b: channel(a.b, b.b),
        a: alpha,
    }
}

#[cfg(test)]
mod mix_tests {
    use super::*;

    #[test]
    fn fading_from_transparent_keeps_the_target_color() {
        let target = rgb(0xe8e2d8);
        let half = mix(transparent(), target, 0.5);
        assert!((half.a - 0.5).abs() < 1e-5);
        assert!((half.r - target.r).abs() < 1e-5);
        assert!((half.g - target.g).abs() < 1e-5);
        assert!((half.b - target.b).abs() < 1e-5);
    }

    #[test]
    fn fading_to_transparent_keeps_the_source_color() {
        let source = rgb(0x203040);
        let quarter = mix(source, transparent(), 0.25);
        assert!((quarter.r - source.r).abs() < 1e-5);
        assert!((quarter.a - 0.75).abs() < 1e-5);
    }

    #[test]
    fn opaque_blends_are_plain_linear_interpolation() {
        let a = rgb(0x000000);
        let b = rgb(0xffffff);
        let mid = mix(a, b, 0.25);
        assert!((mid.r - 0.25).abs() < 1e-5);
        assert!((mid.a - 1.0).abs() < 1e-5);
        assert_eq!(mix(a, b, 0.0).r, 0.0);
        assert_eq!(mix(a, b, 1.0).r, 1.0);
    }

    #[test]
    fn fully_transparent_stays_transparent() {
        assert_eq!(mix(transparent(), transparent(), 0.5).a, 0.0);
        assert_eq!(mix(transparent(), rgb(0xffffff), 0.0).a, 0.0);
    }
}

/// Type sizes. The interface uses six, and hierarchy comes from weight,
/// color, and space rather than another size.
pub mod type_size {
    pub const META: f32 = 12.0;
    pub const SMALL: f32 = 13.0;
    pub const BODY: f32 = 14.0;
    pub const TITLE: f32 = 17.0;
    pub const HEADLINE: f32 = 30.0;
    /// The one hero line on the everyday surface.
    pub const DISPLAY: f32 = 42.0;
    /// The live elapsed time. It is the largest number in the product.
    /// The label on the physical key under the hero line.
    pub const KEYCAP: f32 = 28.0;
    pub const TIMER: f32 = 56.0;
}

/// Line heights are set explicitly everywhere, so no text node inherits a
/// height that depends on the font's opinion.
pub mod line {
    pub const META: f32 = 15.0;
    pub const SMALL: f32 = 17.0;
    pub const BODY: f32 = 21.0;
    pub const TITLE: f32 = 24.0;
    pub const HEADLINE: f32 = 36.0;
    pub const DISPLAY: f32 = 46.0;
    pub const KEYCAP: f32 = 34.0;
    pub const TIMER: f32 = 60.0;
}

/// Instrument Serif, regular weight only. It carries the few lines that matter
/// (the hero line, the section titles, the timer) and is never set bold.
pub fn display_font() -> &'static str {
    "Instrument Serif"
}

/// Instrument Sans, for every other word in the interface.
pub fn text_font() -> &'static str {
    "Instrument Sans"
}

/// A running time such as `0:01.8`, drawn in fixed-width cells. Instrument
/// Serif has no tabular figures, so each digit gets the same width and the
/// number never shivers as it counts.
pub fn timer_digits(text: &str, size: f32, line: f32, color: Rgba) -> gpui::Div {
    use gpui::{ParentElement, Styled, div, px};
    let digit = size * 0.5;
    let separator = size * 0.26;
    div()
        .flex()
        .items_center()
        .h(px(line))
        .font_family(display_font())
        .text_size(px(size))
        .line_height(px(line))
        .text_color(color)
        .children(text.chars().map(move |ch| {
            let width = if ch.is_ascii_digit() {
                digit
            } else {
                separator
            };
            div()
                .w(px(width))
                .flex()
                .justify_center()
                .child(ch.to_string())
        }))
}
