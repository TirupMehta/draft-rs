//! Paste at the cursor on macOS.
//!
//! `Cmd+V` is synthesized with `CGEventPost` at the HID point. Posting events
//! requires Accessibility, which the event tap already asks for, so a build
//! that can hear the dictation key can also send this chord.
//!
//! macOS blocks synthetic input while secure event input is on (a password
//! field, for example). Nothing is lost when that happens: the transcript is
//! on the clipboard and this function reports success, exactly like a paste
//! into an application that ignores the chord.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use arboard::Clipboard;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// `kVK_ANSI_V`: the physical V key, independent of the active layout.
const KEY_V: u16 = 0x09;

/// Left and right variants of every modifier that changes a paste chord.
/// Option matters: Cmd+Option+V is a different command in several editors.
const MODIFIERS: [u16; 8] = [0x37, 0x36, 0x38, 0x3C, 0x3A, 0x3D, 0x3B, 0x3E];

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    /// `CGEventSourceKeyState(CGEventSourceStateID, CGKeyCode)`. Core Graphics
    /// exposes this to C but `core-graphics` 0.24 has no binding for it.
    fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
}

/// Paste `text` at the current focus.
pub fn paste_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    let mut clipboard = Clipboard::new().context("open clipboard")?;
    let previous = clipboard.get_text().ok();
    clipboard
        .set_text(text.to_owned())
        .context("copy transcript to the clipboard")?;

    // Never turn a lingering modifier into an unrelated shortcut. The event
    // tap suppresses the dictation key itself, but a person can still be
    // holding Shift or Option for their own reasons when the result lands.
    if !wait_for_modifiers(Duration::from_millis(500)) {
        bail!("a modifier is still held; the transcript is on the clipboard");
    }

    send_command_v().context("send Cmd+V")?;
    std::thread::sleep(Duration::from_millis(180));

    if let Some(previous) = previous {
        if clipboard.get_text().ok().as_deref() == Some(text) {
            let _ = clipboard.set_text(previous);
        }
    }

    Ok(())
}

fn wait_for_modifiers(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if modifiers_released() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    modifiers_released()
}

fn modifiers_released() -> bool {
    let state = CGEventSourceStateID::HIDSystemState as i32;
    MODIFIERS
        .into_iter()
        .all(|key| unsafe { !CGEventSourceKeyState(state, key) })
}

fn send_command_v() -> Result<()> {
    // A private source marks these events as synthetic, which is what keeps
    // Draft's own event tap from mistaking the chord for a real key press.
    let source = CGEventSource::new(CGEventSourceStateID::Private)
        .map_err(|()| anyhow::anyhow!("create a private event source"))?;

    let down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .map_err(|()| anyhow::anyhow!("create the Cmd+V key-down event"))?;
    down.set_flags(CGEventFlags::CGEventFlagCommand);
    down.post(CGEventTapLocation::HID);

    // Applications expect a human-scale gap between down and up; posting both
    // in the same tick makes some editors miss the chord.
    std::thread::sleep(Duration::from_millis(25));

    let up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .map_err(|()| anyhow::anyhow!("create the Cmd+V key-up event"))?;
    up.set_flags(CGEventFlags::CGEventFlagCommand);
    up.post(CGEventTapLocation::HID);
    Ok(())
}
