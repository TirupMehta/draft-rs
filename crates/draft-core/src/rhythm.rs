//! The learned pause profile.
//!
//! Paragraph breaks are decided relative to the speaker's own rhythm, so the
//! app needs a memory of what that rhythm looks like. Two numbers are enough:
//! the median silence between decoder rows and the 90th percentile. They are
//! measured from real clips, folded in with a slow moving average, and stored
//! next to the settings.
//!
//! This is the only "learning" in the app, and it is deliberately dumb: an
//! average of two integers the engine already reported. Nothing is inferred
//! about what the words mean, nothing leaves the machine, and a user who never
//! dictates twice with a measurable gap simply keeps the zero prior, which
//! makes the paragraph rule fall back to its fixed floor.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// How much a single clip moves the stored average, in percent.
///
/// A slow average is the point: one hurried dictation should not recalibrate
/// the app, and one slow one should not either. Four clips move the median
/// about three quarters of the way to a new habit.
const NEW_CLIP_WEIGHT: i64 = 25;

/// The measured silence profile of one speaker.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rhythm {
    /// Typical silence between decoder rows, in milliseconds. Zero means "not
    /// measured yet", never "no pauses".
    pub median_gap_ms: i64,
    /// The long pauses, which is what a paragraph break is made of.
    pub p90_gap_ms: i64,
    /// How many clips have contributed. Zero means the profile is empty.
    pub clips: u32,
}

impl Rhythm {
    /// `%LOCALAPPDATA%\Draft4\rhythm.json`. Not a user choice, so it does not
    /// belong in `settings.json`.
    pub fn default_path() -> PathBuf {
        crate::paths::data_dir().join("rhythm.json")
    }

    /// Whether anything has been measured. A fresh profile must not be shown as
    /// a number in the interface.
    pub fn is_empty(&self) -> bool {
        self.clips == 0
    }

    /// Read the profile, falling back to an empty one.
    ///
    /// A malformed file is not worth an error dialog: the worst case is that
    /// the first clip of the session calibrates from scratch.
    pub fn load_or_default(path: &Path) -> Self {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(%error, "could not read the pause profile; starting over");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Write the profile atomically, like the settings file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .with_context(|| format!("create profile directory {}", parent.display()))?;

        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("create a temporary file in {}", parent.display()))?;
        serde_json::to_writer_pretty(&mut temporary, self)
            .context("serialize the pause profile")?;
        temporary
            .write_all(b"\n")
            .context("finish the profile file")?;
        temporary
            .as_file()
            .sync_all()
            .context("sync the profile file")?;
        temporary
            .persist(path)
            .map_err(|error| error.error)
            .with_context(|| format!("replace the profile at {}", path.display()))?;
        Ok(())
    }

    /// Fold one clip's measurement into the profile.
    ///
    /// Negative values are clamped: a decoder row that starts before the last
    /// one ended describes no silence at all, and averaging a negative gap into
    /// the profile would drag the paragraph threshold below its floor.
    pub fn observe(&mut self, median_gap_ms: i64, p90_gap_ms: i64) {
        let median = median_gap_ms.max(0);
        let p90 = p90_gap_ms.max(0);

        if self.clips == 0 {
            self.median_gap_ms = median;
            self.p90_gap_ms = p90;
        } else {
            self.median_gap_ms = blend(self.median_gap_ms, median);
            self.p90_gap_ms = blend(self.p90_gap_ms, p90);
        }
        self.clips = self.clips.saturating_add(1);
    }
}

/// A slow average that stays in integer milliseconds.
fn blend(previous: i64, measured: i64) -> i64 {
    let kept = previous.saturating_mul(100 - NEW_CLIP_WEIGHT);
    let added = measured.saturating_mul(NEW_CLIP_WEIGHT);
    (kept + added) / 100
}

#[cfg(test)]
mod tests {
    use super::Rhythm;

    #[test]
    fn an_empty_profile_is_not_a_measurement() {
        let rhythm = Rhythm::default();
        assert!(rhythm.is_empty());
        assert_eq!(rhythm.median_gap_ms, 0);
    }

    #[test]
    fn the_first_clip_is_taken_as_is() {
        let mut rhythm = Rhythm::default();
        rhythm.observe(820, 1_400);
        assert_eq!(rhythm.median_gap_ms, 820);
        assert_eq!(rhythm.p90_gap_ms, 1_400);
        assert_eq!(rhythm.clips, 1);
    }

    #[test]
    fn later_clips_move_the_average_slowly() {
        let mut rhythm = Rhythm::default();
        rhythm.observe(800, 1_200);
        rhythm.observe(1_200, 1_600);
        // A quarter of the way from 800 to 1200.
        assert_eq!(rhythm.median_gap_ms, 900);
        assert_eq!(rhythm.p90_gap_ms, 1_300);
        assert_eq!(rhythm.clips, 2);

        rhythm.observe(1_200, 1_600);
        rhythm.observe(1_200, 1_600);
        rhythm.observe(1_200, 1_600);
        // Converges on the new habit without ever jumping to it.
        assert!(rhythm.median_gap_ms > 1_000);
        assert!(rhythm.median_gap_ms < 1_200);
    }

    #[test]
    fn negative_measurements_never_reach_the_profile() {
        let mut rhythm = Rhythm::default();
        rhythm.observe(-40, -10);
        assert_eq!(rhythm.median_gap_ms, 0);
        assert_eq!(rhythm.p90_gap_ms, 0);
    }

    #[test]
    fn round_trips_through_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rhythm.json");
        let mut rhythm = Rhythm::default();
        rhythm.observe(640, 1_100);
        rhythm.save(&path).unwrap();
        assert_eq!(Rhythm::load_or_default(&path), rhythm);
    }

    #[test]
    fn a_malformed_file_starts_over_instead_of_failing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rhythm.json");
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(Rhythm::load_or_default(&path), Rhythm::default());
    }
}
