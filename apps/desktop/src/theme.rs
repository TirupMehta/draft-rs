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
//! * Dark is the default because Draft floats over other applications. The
//!   graphite is warm, not blue-black, so the amber sits in the same room.
//! * Light is designed, not inverted: warm paper, deeper amber, the same
//!   structure and the same restraint.

use gpui::{Rgba, px, rgb, rgba};

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
pub fn mix(a: Rgba, b: Rgba, amount: f32) -> Rgba {
    let amount = amount.clamp(0.0, 1.0);
    Rgba {
        r: a.r + (b.r - a.r) * amount,
        g: a.g + (b.g - a.g) * amount,
        b: a.b + (b.b - a.b) * amount,
        a: a.a + (b.a - a.a) * amount,
    }
}

/// Type sizes. The interface uses six, and hierarchy comes from weight,
/// color, and space rather than another size.
pub mod type_size {
    pub const META: f32 = 12.0;
    pub const SMALL: f32 = 13.0;
    pub const BODY: f32 = 14.0;
    pub const TITLE: f32 = 17.0;
    pub const HEADLINE: f32 = 26.0;
    /// The one hero line on the everyday surface.
    pub const DISPLAY: f32 = 34.0;
    /// The live elapsed time. It is the largest number in the product.
    pub const TIMER: f32 = 48.0;
}

/// Line heights are set explicitly everywhere, so no text node inherits a
/// height that depends on the font's opinion.
pub mod line {
    pub const META: f32 = 15.0;
    pub const SMALL: f32 = 17.0;
    pub const BODY: f32 = 21.0;
    pub const TITLE: f32 = 24.0;
    pub const HEADLINE: f32 = 32.0;
    pub const DISPLAY: f32 = 40.0;
    pub const TIMER: f32 = 54.0;
}

pub fn display_font() -> &'static str {
    "Inter"
}

pub fn text_font() -> &'static str {
    "Inter"
}

pub fn mono_font() -> &'static str {
    "JetBrains Mono"
}

/// The HUD floats over other applications, so it gets one soft shadow to lift
/// it off whatever is behind it. It is the only shadow in the product.
pub fn hud_shadow() -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: gpui::hsla(0.0, 0.0, 0.0, 0.42),
        offset: gpui::point(px(0.0), px(10.0)),
        blur_radius: px(26.0),
        spread_radius: px(-8.0),
    }]
}
