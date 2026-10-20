//! Global hold-to-talk hotkey.
//!
//! Windows uses a low-level keyboard hook so a bare modifier such as Right
//! Ctrl can start and stop a capture. The hook observes the physical key,
//! ignores injected events, and suppresses the dictation key while it is held
//! so the focused app never sees a stray modifier or Escape.

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyConfig {
    /// Windows virtual-key code. Defaults to Right Ctrl (`0xA3`).
    pub virtual_key: u32,
    /// Human-readable name for the UI.
    pub name: &'static str,
}

impl HotkeyConfig {
    /// Resolve a short configuration name such as `right-ctrl` or `f8`.
    pub fn named(name: &str) -> Option<Self> {
        let normalized = name.trim().to_ascii_lowercase().replace(['-', '_'], " ");
        let (virtual_key, name) = match normalized.as_str() {
            "right ctrl" | "right control" | "rctrl" => (VK_RCONTROL, "Right Ctrl"),
            "right alt" | "ralt" | "altgr" => (VK_RMENU, "Right Alt"),
            "f8" => (VK_F8, "F8"),
            "f9" => (VK_F9, "F9"),
            "pause" => (VK_PAUSE, "Pause"),
            "scroll lock" | "scrolllock" => (VK_SCROLL, "Scroll Lock"),
            _ => return None,
        };
        Some(Self { virtual_key, name })
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            virtual_key: VK_RCONTROL,
            name: "Right Ctrl",
        }
    }
}

/// Right Ctrl, the default dictation key.
pub const VK_RCONTROL: u32 = 0xA3;
/// Right Alt / AltGr.
pub const VK_RMENU: u32 = 0xA5;
/// Escape, which cancels an active capture.
pub const VK_ESCAPE: u32 = 0x1B;
/// F8, useful while another dictation app owns Right Ctrl.
pub const VK_F8: u32 = 0x77;
/// F9.
pub const VK_F9: u32 = 0x78;
/// Pause/Break.
pub const VK_PAUSE: u32 = 0x13;
/// Scroll Lock.
pub const VK_SCROLL: u32 = 0x91;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::HotkeyListener;

#[cfg(windows)]
pub fn start(config: HotkeyConfig) -> anyhow::Result<(HotkeyListener, Receiver<HotkeyEvent>)> {
    windows::start(config)
}

#[cfg(not(windows))]
pub struct HotkeyListener;

#[cfg(not(windows))]
pub fn start(_config: HotkeyConfig) -> anyhow::Result<(HotkeyListener, Receiver<HotkeyEvent>)> {
    anyhow::bail!("the global hotkey is implemented on Windows first")
}
