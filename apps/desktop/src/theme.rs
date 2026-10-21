//! The Draft design system.
//!
//! Two palettes, one structure. Every color, radius, and size the interface
//! uses comes from here, so the product reads as a single object instead of a
//! collection of decorated panels.
//!
//! The rules this file encodes:
//!
//! * One accent. Amber marks live state (recording, selection, focus).
//! * Hierarchy comes from type and space. Containers are the exception.
//! * Dark is the default because Draft is a floating utility that sits over
//!   other applications. Light is designed, not inverted: warmer paper
//!   background, deeper accent for contrast, and the same structure.

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
    /// The window background. Nothing sits on it except content.
    pub canvas: Rgba,
    /// Inputs, switch tracks, and segmented tracks.
    pub raised: Rgba,
    /// The hover state of a raised surface.
    pub raised_hover: Rgba,
    /// A quiet visible line, used for meter baselines and empty tracks.
    pub track: Rgba,
    /// Hairlines between rows.
    pub hairline: Rgba,
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
    /// A keyboard key.
    pub keycap: Rgba,
    pub keycap_border: Rgba,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            canvas: rgb(0x0d0d0f),
            raised: rgb(0x1b1b20),
            raised_hover: rgb(0x24242b),
            track: rgb(0x2a2a32),
            hairline: rgba(0xffffff14),
            text: rgb(0xf3f3f5),
            secondary: rgb(0xa3a3ac),
            tertiary: rgb(0x74747e),
            accent: rgb(0xe8a33c),
            accent_soft: rgba(0xe8a33c1f),
            accent_text: rgb(0xf0b95f),
            danger: rgb(0xe2685c),
            danger_soft: rgba(0xe2685c1f),
            keycap: rgb(0x1e1e24),
            keycap_border: rgba(0xffffff1f),
        }
    }

    pub fn light() -> Self {
        Self {
            canvas: rgb(0xfaf9f7),
            raised: rgb(0xffffff),
            raised_hover: rgb(0xf0efeb),
            track: rgb(0xe4e2dd),
            hairline: rgba(0x00000016),
            text: rgb(0x151517),
            secondary: rgb(0x5c5c65),
            tertiary: rgb(0x8b8b93),
            accent: rgb(0xa86510),
            accent_soft: rgba(0xa8651014),
            accent_text: rgb(0x8b5409),
            danger: rgb(0xb03a2e),
            danger_soft: rgba(0xb03a2e12),
            keycap: rgb(0xffffff),
            keycap_border: rgba(0x00000024),
        }
    }
}

/// Spacing. Everything is a multiple of four.
pub mod space {
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
}

/// Corner radii. Three values only: 6 px for controls, 12 px for surfaces,
/// and a capsule for the switch.
pub mod radius {
    pub const CONTROL: f32 = 6.0;
    pub const PANEL: f32 = 12.0;
    pub const CAPSULE: f32 = 9999.0;
}

const _: () = {
    // The radii are documented here and applied with GPUI's radius scale.
    let _ = (radius::CONTROL, radius::PANEL, radius::CAPSULE);
};

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

/// Type sizes. The interface uses five.
pub mod type_size {
    pub const META: f32 = 12.0;
    pub const SMALL: f32 = 13.0;
    pub const BODY: f32 = 14.0;
    pub const TITLE: f32 = 16.0;
    pub const HEADLINE: f32 = 21.0;
}

pub fn display_font() -> &'static str {
    "Segoe UI Variable Display"
}

pub fn text_font() -> &'static str {
    "Segoe UI Variable Text"
}

/// The HUD floats over other applications, so it gets one soft shadow to lift
/// it off whatever is behind it.
pub fn hud_shadow() -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: gpui::hsla(0.0, 0.0, 0.0, 0.34),
        offset: gpui::point(px(0.0), px(8.0)),
        blur_radius: px(18.0),
        spread_radius: px(-6.0),
    }]
}
