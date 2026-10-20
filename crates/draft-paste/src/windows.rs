use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use arboard::Clipboard;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL,
    VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_V,
};

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

    // Never turn a lingering Shift or Ctrl into an unrelated shortcut. The
    // dictation key is suppressed by the hook, but other modifiers may still
    // be physically down when the user releases the capture.
    if !wait_for_modifiers(Duration::from_millis(500)) {
        bail!("a modifier is still held; the transcript is on the clipboard");
    }

    send_ctrl_v().context("send Ctrl+V")?;
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
    [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
        .into_iter()
        .all(|key| unsafe { GetAsyncKeyState(i32::from(key)) } >= 0)
}

fn send_ctrl_v() -> Result<()> {
    let inputs = [
        key_event(VK_CONTROL, false),
        key_event(VK_V, false),
        key_event(VK_V, true),
        key_event(VK_CONTROL, true),
    ];
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent != inputs.len() as u32 {
        bail!("SendInput accepted {sent} of {} events", inputs.len());
    }
    Ok(())
}

fn key_event(vk: u16, up: bool) -> INPUT {
    let mut input: INPUT = unsafe { std::mem::zeroed() };
    input.r#type = INPUT_KEYBOARD;
    input.Anonymous.ki = KEYBDINPUT {
        wVk: vk,
        wScan: 0,
        dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
        time: 0,
        dwExtraInfo: 0,
    };
    input
}
