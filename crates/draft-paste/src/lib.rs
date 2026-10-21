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

#[cfg(not(windows))]
pub fn paste_text(_text: &str) -> anyhow::Result<()> {
    anyhow::bail!("paste is implemented on Windows first")
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
