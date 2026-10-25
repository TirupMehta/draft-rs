//! Global hold-to-talk hotkey.
//!
//! The public surface here is platform-neutral: a small set of named keys, the
//! events they produce, and the listener handle. Each platform translates the
//! name into whatever its input API needs.
//!
//! Windows uses a low-level keyboard hook so a bare modifier such as Right Ctrl
//! can start and stop a capture. The hook observes the physical key, ignores
//! injected events, and suppresses the dictation key while it is held so the
//! focused app never sees a stray modifier or Escape.
//!
//! macOS uses an active `CGEventTap` for the same job; modifiers arrive there
//! as flag changes rather than key events. Both platforms ignore synthetic
//! input, so Draft's own paste chord cannot start a capture.

use std::sync::mpsc::Receiver;

/// A physical event from the dictation key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// The dictation key went down.
    Pressed,
    /// The dictation key came up.
    Released,
    /// Escape was pressed while the dictation key was held.
    Cancel,
}

/// Which physical key starts dictation.
///
/// The variant, not a virtual-key code, is the source of truth. Settings store
/// `id()`, and each platform maps the variant to its own key code. That keeps
/// `settings.json` portable between Windows and macOS, where the same physical
/// key has a different numeric code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HotkeyConfig {
    /// Right Ctrl. On macOS this is the right Control key.
    #[default]
    RightCtrl,
    /// Right Alt / AltGr. On macOS this is the right Option key.
    RightAlt,
    F8,
    F9,
    Pause,
    ScrollLock,
}

impl HotkeyConfig {
    /// Every key the interface offers, in display order.
    pub const ALL: [Self; 6] = [
        Self::RightCtrl,
        Self::RightAlt,
        Self::F8,
        Self::F9,
        Self::Pause,
        Self::ScrollLock,
    ];

    /// The keys the current platform actually has.
    ///
    /// Apple keyboards have no Pause and no Scroll Lock, so macOS offers four
    /// choices instead of six. Listing a key that cannot be pressed would be a
    /// setting that silently never fires.
    pub fn available() -> &'static [Self] {
        #[cfg(target_os = "macos")]
        {
            &[Self::RightCtrl, Self::RightAlt, Self::F8, Self::F9]
        }
        #[cfg(not(target_os = "macos"))]
        {
            &Self::ALL
        }
    }

    /// Resolve a short configuration name such as `right-ctrl` or `f8`.
    ///
    /// Aliases exist because the setting has been written by hand, by the CLI,
    /// and by the app over time; a stricter parser would silently drop a key
    /// the person had chosen.
    pub fn named(name: &str) -> Option<Self> {
        let normalized = name.trim().to_ascii_lowercase().replace(['-', '_'], " ");
        match normalized.as_str() {
            "right ctrl" | "right control" | "rctrl" => Some(Self::RightCtrl),
            "right alt" | "ralt" | "altgr" | "right option" | "roption" => Some(Self::RightAlt),
            "f8" => Some(Self::F8),
            "f9" => Some(Self::F9),
            "pause" | "break" => Some(Self::Pause),
            "scroll lock" | "scrolllock" => Some(Self::ScrollLock),
            _ => None,
        }
    }

    /// The stable, lowercase identifier written to settings.
    pub fn id(self) -> &'static str {
        match self {
            Self::RightCtrl => "right-ctrl",
            Self::RightAlt => "right-alt",
            Self::F8 => "f8",
            Self::F9 => "f9",
            Self::Pause => "pause",
            Self::ScrollLock => "scroll-lock",
        }
    }

    /// Human-readable name for the UI.
    pub fn name(self) -> &'static str {
        match self {
            Self::RightCtrl => "Right Ctrl",
            Self::RightAlt => "Right Alt",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::Pause => "Pause",
            Self::ScrollLock => "Scroll Lock",
        }
    }
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::HotkeyListener;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::HotkeyListener;

#[cfg(windows)]
pub fn start(config: HotkeyConfig) -> anyhow::Result<(HotkeyListener, Receiver<HotkeyEvent>)> {
    windows::start(config)
}

#[cfg(target_os = "macos")]
pub fn start(config: HotkeyConfig) -> anyhow::Result<(HotkeyListener, Receiver<HotkeyEvent>)> {
    macos::start(config)
}

#[cfg(not(any(windows, target_os = "macos")))]
pub struct HotkeyListener;

#[cfg(not(any(windows, target_os = "macos")))]
pub fn start(_config: HotkeyConfig) -> anyhow::Result<(HotkeyListener, Receiver<HotkeyEvent>)> {
    anyhow::bail!("the global hotkey is not implemented on this platform yet")
}

#[cfg(test)]
mod tests {
    use super::HotkeyConfig;

    #[test]
    fn every_key_round_trips_through_its_id() {
        for config in HotkeyConfig::ALL {
            assert_eq!(
                HotkeyConfig::named(config.id()),
                Some(config),
                "{} did not round-trip",
                config.id()
            );
        }
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = HotkeyConfig::ALL.iter().map(|config| config.id()).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before, "two keys share an id");
    }

    #[test]
    fn aliases_resolve_to_the_same_key() {
        for (alias, expected) in [
            ("right control", HotkeyConfig::RightCtrl),
            ("RCTRL", HotkeyConfig::RightCtrl),
            ("right_option", HotkeyConfig::RightAlt),
            ("AltGr", HotkeyConfig::RightAlt),
            ("Break", HotkeyConfig::Pause),
            ("scroll_lock", HotkeyConfig::ScrollLock),
        ] {
            assert_eq!(HotkeyConfig::named(alias), Some(expected), "alias {alias}");
        }
    }

    #[test]
    fn unknown_names_are_rejected() {
        assert_eq!(HotkeyConfig::named("left-ctrl"), None);
        assert_eq!(HotkeyConfig::named(""), None);
        assert_eq!(HotkeyConfig::named("f10"), None);
    }

    #[test]
    fn the_default_is_the_shipped_default() {
        assert_eq!(HotkeyConfig::default(), HotkeyConfig::RightCtrl);
        assert_eq!(HotkeyConfig::default().id(), "right-ctrl");
    }

    #[test]
    fn every_offered_key_is_available() {
        for config in HotkeyConfig::available() {
            assert!(
                HotkeyConfig::ALL.contains(config),
                "{} is offered but not known",
                config.id()
            );
        }
    }
}
