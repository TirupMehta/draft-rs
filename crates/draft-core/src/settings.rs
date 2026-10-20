//! Persisted, user-facing settings.
//!
//! The core owns the schema and the atomic file write, but it does not know
//! which keys exist or which microphone names are valid. The desktop shell
//! validates those against its live platform state before saving.
//!
//! Missing fields use the default value, so adding a field is a compatible
//! change for older files. Unknown fields are ignored for the same reason.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The user's persisted choices. `None` means "use the platform default".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// A short name understood by the platform hotkey layer, for example
    /// `right-ctrl` or `f8`.
    pub hotkey: String,
    /// An absolute path to a local GGUF model. `None` uses the app default.
    pub model: Option<PathBuf>,
    /// The preferred input device name. `None` uses the system default.
    pub microphone: Option<String>,
    /// Whether a finished transcript is pasted at the cursor.
    pub paste: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "right-ctrl".to_owned(),
            model: None,
            microphone: None,
            paste: true,
        }
    }
}

impl Settings {
    /// The one settings file, next to the model cache.
    ///
    /// `data_local_dir` is `%LOCALAPPDATA%` on Windows,
    /// `~/Library/Application Support` on macOS, and `~/.local/share` on
    /// Linux. Those are the right per-user, non-roaming locations for a
    /// local-first app.
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Draft")
            .join("settings.json")
    }

    /// Read settings from `path`. A missing file is not an error; a malformed
    /// file is, so the caller can warn without silently overwriting it.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        match fs::read(path) {
            Ok(bytes) => {
                let settings = serde_json::from_slice(&bytes)
                    .with_context(|| format!("parse settings at {}", path.display()))?;
                Ok(Some(settings))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => {
                Err(error).with_context(|| format!("read settings at {}", path.display()))
            }
        }
    }

    /// Load the file, or return defaults when it is absent or unreadable.
    ///
    /// A malformed file is left on disk for the user to inspect; the next
    /// successful save replaces it.
    pub fn load_or_default(path: &Path) -> Self {
        match Self::load(path) {
            Ok(Some(settings)) => settings,
            Ok(None) => Self::default(),
            Err(error) => {
                tracing::warn!(%error, "could not read settings; using defaults");
                Self::default()
            }
        }
    }

    /// Write the settings atomically. Readers see either the old file or the
    /// complete new file, never a half-written one.
    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .with_context(|| format!("create settings directory {}", parent.display()))?;

        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("create a temporary file in {}", parent.display()))?;
        serde_json::to_writer_pretty(&mut temporary, self).context("serialize settings")?;
        temporary.write_all(b"\n").context("finish settings file")?;
        temporary
            .as_file()
            .sync_all()
            .context("sync settings file")?;
        temporary
            .persist(path)
            .map_err(|error| error.error)
            .with_context(|| format!("replace settings at {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn defaults_are_the_shipping_choices() {
        let settings = Settings::default();
        assert_eq!(settings.hotkey, "right-ctrl");
        assert_eq!(settings.model, None);
        assert_eq!(settings.microphone, None);
        assert!(settings.paste);
    }

    #[test]
    fn missing_file_is_not_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        assert!(Settings::load(&path).unwrap().is_none());
        assert_eq!(Settings::load_or_default(&path), Settings::default());
    }

    #[test]
    fn round_trip_preserves_every_field() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested").join("settings.json");
        let settings = Settings {
            hotkey: "f8".to_owned(),
            model: Some(directory.path().join("models/turbo.gguf")),
            microphone: Some("USB Microphone".to_owned()),
            paste: false,
        };

        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), Some(settings));
        assert!(
            directory
                .path()
                .join("nested")
                .read_dir()
                .unwrap()
                .all(|entry| entry.unwrap().file_name() == "settings.json"),
            "the save must not leave temporary files behind"
        );
    }

    #[test]
    fn missing_fields_use_defaults_and_unknown_fields_are_ignored() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(
            &path,
            br#"{"hotkey":"f9","microphone":"Desk Mic","future_option":true}"#,
        )
        .unwrap();

        let settings = Settings::load(&path).unwrap().unwrap();
        assert_eq!(settings.hotkey, "f9");
        assert_eq!(settings.microphone.as_deref(), Some("Desk Mic"));
        assert_eq!(settings.model, None);
        assert!(settings.paste);
    }

    #[test]
    fn malformed_file_is_reported_and_survives_until_the_next_save() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(&path, b"not json").unwrap();

        assert!(Settings::load(&path).is_err());
        assert_eq!(Settings::load_or_default(&path), Settings::default());
        assert_eq!(std::fs::read(&path).unwrap(), b"not json");

        Settings::default().save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), Some(Settings::default()));
    }
}
