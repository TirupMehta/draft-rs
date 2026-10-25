//! The one per-user data folder for this rewrite.
//!
//! Draft v3.5 keeps `%LOCALAPPDATA%\Draft` (its `config.json` and
//! `history.json` live there). The rewrite intentionally uses a different
//! folder so the shipping app's settings, history, and logs are never read,
//! migrated, or overwritten. A deliberate migration can come later; silence
//! now is safer than a collision.

use std::path::PathBuf;

use crate::models;

/// Folder name under the platform's local data directory.
pub const DATA_DIR_NAME: &str = "Draft4";

/// `%LOCALAPPDATA%\Draft4` on Windows, `~/Library/Application Support/Draft4`
/// on macOS, and `~/.local/share/Draft4` on Linux.
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DATA_DIR_NAME)
}

/// The data folder as a person would type it, for interface copy. It stays the
/// same on every platform the app ships to, so the settings pane tells the
/// truth instead of naming a Windows variable on a Mac.
pub fn data_dir_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "%LOCALAPPDATA%\\Draft4"
    } else if cfg!(target_os = "macos") {
        "~/Library/Application Support/Draft4"
    } else {
        "~/.local/share/Draft4"
    }
}

/// Installed GGUF files.
pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

/// Where a fresh install expects the shipping model.
pub fn default_model_path() -> PathBuf {
    models_dir().join(models::DEFAULT_MODEL.file_name)
}

/// The retained dictation history file.
pub fn history_path() -> PathBuf {
    std::env::var_os("DRAFT_HISTORY_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir().join("history.json"))
}
