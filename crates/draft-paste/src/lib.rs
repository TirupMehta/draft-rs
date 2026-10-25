//! Insert a finished transcript into the focused application.
//!
//! Draft copies the text, sends the platform's paste chord, and restores the
//! previous clipboard text only if nothing else replaced the transcript. The
//! transcript stays in the clipboard when there was no previous text, so a
//! failed paste is never a lost result.

use anyhow::Context;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::paste_text;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::paste_text;

#[cfg(not(any(windows, target_os = "macos")))]
pub fn paste_text(_text: &str) -> anyhow::Result<()> {
    anyhow::bail!("paste is not implemented on this platform yet")
}

/// Put a transcript on the clipboard without sending a paste chord. The
/// interface uses it for the "Copy" action on the last dictation.
pub fn copy_text(text: &str) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("open the clipboard")?;
    clipboard
        .set_text(text.to_owned())
        .context("copy the text")?;
    Ok(())
}
