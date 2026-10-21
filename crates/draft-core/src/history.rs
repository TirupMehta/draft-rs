//! Retained dictation history.
//!
//! Text and bounded metadata only: no audio, no window titles, no URLs. The
//! store is one JSON file rewritten atomically, so a crash leaves either the
//! previous history or the complete new one.
//!
//! Three limits always apply together: a retention window, an entry cap, and a
//! byte cap. Whichever is tightest wins. Turning retention off stops new
//! entries without deleting what is already there; deleting is always explicit.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The format version written to disk.
const VERSION: u32 = 1;

/// Estimated bytes each entry costs beyond its text (ids, timestamps, JSON).
const ENTRY_OVERHEAD: u64 = 96;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// How long entries stay eligible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Retention {
    /// Keep existing entries, accept no new ones.
    Off,
    /// Keep entries newer than this many days.
    Days(u32),
    /// Keep entries until a cap forces eviction.
    Forever,
}

impl Default for Retention {
    fn default() -> Self {
        Self::Days(30)
    }
}

/// The three limits applied together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPolicy {
    pub retention: Retention,
    pub max_entries: usize,
    pub max_bytes: u64,
}

impl Default for HistoryPolicy {
    fn default() -> Self {
        Self {
            retention: Retention::Days(30),
            max_entries: 2_000,
            max_bytes: 8 * 1024 * 1024,
        }
    }
}

impl HistoryPolicy {
    /// Whether new entries may be stored at all.
    pub fn accepts_new_entries(&self) -> bool {
        !matches!(self.retention, Retention::Off)
    }

    fn is_expired(&self, created_at_ms: i64, now_ms: i64) -> bool {
        match self.retention {
            Retention::Off | Retention::Forever => false,
            Retention::Days(days) => {
                let window = i64::from(days).saturating_mul(86_400_000);
                created_at_ms.saturating_add(window) <= now_ms
            }
        }
    }
}

/// One retained dictation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub created_at_ms: i64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl HistoryEntry {
    /// Approximate on-disk cost.
    pub fn cost(&self) -> u64 {
        self.text.len() as u64 + self.id.len() as u64 + ENTRY_OVERHEAD
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileFormat {
    version: u32,
    entries: Vec<HistoryEntry>,
}

/// The current wall clock in milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_millis() as i64,
        Err(_) => 0,
    }
}

/// The owner of one history file.
#[derive(Debug)]
pub struct History {
    path: PathBuf,
    policy: HistoryPolicy,
    entries: Vec<HistoryEntry>,
    /// Set when the file was unreadable and quarantined, so the caller can warn.
    pub recovered_from_corruption: bool,
}

impl History {
    /// Open `path`, pruning expired and over-cap entries.
    ///
    /// A missing file is an empty history. A malformed file is moved aside to
    /// `<name>.corrupt-<millis>.json` rather than silently overwritten, and the
    /// store starts empty with [`recovered_from_corruption`](Self::recovered_from_corruption)
    /// set.
    pub fn open(path: PathBuf, policy: HistoryPolicy, now_ms: i64) -> Result<Self> {
        let mut history = Self {
            path,
            policy,
            entries: Vec::new(),
            recovered_from_corruption: false,
        };

        match fs::read(&history.path) {
            Ok(bytes) => match serde_json::from_slice::<FileFormat>(&bytes) {
                Ok(file) => history.entries = file.entries,
                Err(error) => {
                    tracing::warn!(
                        path = %history.path.display(),
                        %error,
                        "history file is malformed; quarantining it"
                    );
                    history.quarantine(now_ms)?;
                    history.recovered_from_corruption = true;
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read history at {}", history.path.display()));
            }
        }

        history.entries.sort_by_key(|entry| entry.created_at_ms);
        history.prune(now_ms);
        Ok(history)
    }

    /// The file this store owns.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The active policy.
    pub fn policy(&self) -> HistoryPolicy {
        self.policy
    }

    /// Every entry, oldest first.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Store one transcript. Returns `None` when retention is off, when the
    /// text is empty, or when storage failed after pruning.
    pub fn add(
        &mut self,
        text: &str,
        duration_ms: Option<u64>,
        now_ms: i64,
    ) -> Result<Option<HistoryEntry>> {
        if !self.policy.accepts_new_entries() {
            return Ok(None);
        }
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }

        let entry = HistoryEntry {
            id: new_id(now_ms),
            created_at_ms: now_ms,
            text: text.to_owned(),
            duration_ms,
        };
        self.entries.push(entry.clone());
        self.prune(now_ms);
        self.save()?;
        Ok(Some(entry))
    }

    /// Newest first, at most `limit`.
    pub fn recent(&self, limit: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(limit).collect()
    }

    /// Newest first, case-insensitive substring match.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&HistoryEntry> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return self.recent(limit);
        }
        self.entries
            .iter()
            .rev()
            .filter(|entry| entry.text.to_lowercase().contains(&needle))
            .take(limit)
            .collect()
    }

    /// Delete one entry by id. Returns whether it existed.
    pub fn delete(&mut self, id: &str) -> Result<bool> {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.id != id);
        let removed = self.entries.len() != before;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Delete every entry, keeping the file.
    pub fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        self.save()
    }

    /// Apply the retention window, the entry cap, and the byte cap.
    fn prune(&mut self, now_ms: i64) {
        self.entries
            .retain(|entry| !self.policy.is_expired(entry.created_at_ms, now_ms));

        while self.entries.len() > self.policy.max_entries {
            self.entries.remove(0);
        }

        let mut bytes: u64 = self.entries.iter().map(HistoryEntry::cost).sum();
        while bytes > self.policy.max_bytes && !self.entries.is_empty() {
            let removed = self.entries.remove(0);
            bytes = bytes.saturating_sub(removed.cost());
        }
    }

    fn save(&self) -> Result<()> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .with_context(|| format!("create the history folder {}", parent.display()))?;

        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("create a temporary file in {}", parent.display()))?;
        let file = FileFormat {
            version: VERSION,
            entries: self.entries.clone(),
        };
        serde_json::to_writer(&mut temporary, &file).context("serialize history")?;
        temporary.write_all(b"\n").context("finish history file")?;
        temporary
            .as_file()
            .sync_all()
            .context("sync history file")?;
        temporary
            .persist(&self.path)
            .map_err(|error| error.error)
            .with_context(|| format!("replace history at {}", self.path.display()))?;
        Ok(())
    }

    fn quarantine(&mut self, now_ms: i64) -> Result<()> {
        let backup = self
            .path
            .with_file_name(format!("history.corrupt-{now_ms}.json"));
        fs::rename(&self.path, &backup)
            .with_context(|| format!("move the malformed history to {}", backup.display()))?;
        self.entries.clear();
        Ok(())
    }
}

fn new_id(now_ms: i64) -> String {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{now_ms:013x}-{sequence:04x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000;

    fn store(dir: &Path, policy: HistoryPolicy, now: i64) -> History {
        History::open(dir.join("history.json"), policy, now).unwrap()
    }

    #[test]
    fn defaults_are_bounded_and_private() {
        let policy = HistoryPolicy::default();
        assert_eq!(policy.retention, Retention::Days(30));
        assert_eq!(policy.max_entries, 2_000);
        assert_eq!(policy.max_bytes, 8 * 1024 * 1024);
        assert!(policy.accepts_new_entries());
    }

    #[test]
    fn added_entries_survive_a_reopen_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        {
            let mut history = store(dir.path(), HistoryPolicy::default(), now);
            history.add("first words", Some(900), now - 2_000).unwrap();
            history.add("second words", None, now - 1_000).unwrap();
        }
        let history = store(dir.path(), HistoryPolicy::default(), now);
        let recent = history.recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].text, "second words");
        assert_eq!(recent[1].text, "first words");
        assert_eq!(recent[1].duration_ms, Some(900));
    }

    #[test]
    fn unicode_and_multiline_text_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        let text = "Héllo, 世界!\nSecond line — dash.";
        {
            let mut history = store(dir.path(), HistoryPolicy::default(), now);
            history.add(text, None, now).unwrap();
        }
        let history = store(dir.path(), HistoryPolicy::default(), now);
        assert_eq!(history.recent(1)[0].text, text);
    }

    #[test]
    fn empty_text_is_not_stored() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        let mut history = store(dir.path(), HistoryPolicy::default(), now);
        assert!(history.add("   \n\t ", None, now).unwrap().is_none());
        assert!(history.entries().is_empty());
    }

    #[test]
    fn off_retention_refuses_new_entries_but_keeps_old_ones() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        {
            let mut history = store(dir.path(), HistoryPolicy::default(), now);
            history.add("keep me", None, now).unwrap();
        }

        let policy = HistoryPolicy {
            retention: Retention::Off,
            ..HistoryPolicy::default()
        };
        let mut history = store(dir.path(), policy, now + DAY);
        assert!(
            history
                .add("never stored", None, now + DAY)
                .unwrap()
                .is_none()
        );
        assert_eq!(history.recent(10).len(), 1);
        assert_eq!(history.recent(10)[0].text, "keep me");
    }

    #[test]
    fn entries_expire_after_the_retention_window() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        let policy = HistoryPolicy {
            retention: Retention::Days(7),
            ..HistoryPolicy::default()
        };
        {
            let mut history = store(dir.path(), policy, now - 8 * DAY);
            history.add("old", None, now - 8 * DAY).unwrap();
        }
        let history = store(dir.path(), policy, now);
        assert!(
            history.entries().is_empty(),
            "expired entries must not load"
        );
    }

    #[test]
    fn forever_keeps_entries_past_the_default_window() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        let policy = HistoryPolicy {
            retention: Retention::Forever,
            ..HistoryPolicy::default()
        };
        let mut history = store(dir.path(), policy, now);
        history.add("ancient", None, now - 3_650 * DAY).unwrap();
        assert_eq!(history.recent(1)[0].text, "ancient");
    }

    #[test]
    fn the_entry_cap_evicts_the_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        let policy = HistoryPolicy {
            max_entries: 2,
            ..HistoryPolicy::default()
        };
        let mut history = store(dir.path(), policy, now);
        for index in 0..4 {
            history
                .add(&format!("entry {index}"), None, now + index)
                .unwrap();
        }
        let recent = history.recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].text, "entry 3");
        assert_eq!(recent[1].text, "entry 2");
    }

    #[test]
    fn the_byte_cap_evicts_the_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        let policy = HistoryPolicy {
            max_bytes: 400,
            max_entries: 100,
            ..HistoryPolicy::default()
        };
        let mut history = store(dir.path(), policy, now);
        for index in 0..5 {
            history.add(&"x".repeat(100), None, now + index).unwrap();
        }
        let bytes: u64 = history.entries().iter().map(HistoryEntry::cost).sum();
        assert!(bytes <= policy.max_bytes, "kept {bytes} bytes");
        assert!(history.entries().len() < 5);
    }

    #[test]
    fn search_is_case_insensitive_and_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        let mut history = store(dir.path(), HistoryPolicy::default(), now);
        history
            .add("Deploy the STAGING build", None, now - 3)
            .unwrap();
        history.add("unrelated", None, now - 2).unwrap();
        history.add("staging again", None, now - 1).unwrap();

        let hits = history.search("staging", 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].text, "staging again");
        assert!(history.search("missing", 10).is_empty());
        assert_eq!(history.search("   ", 1).len(), 1);
    }

    #[test]
    fn delete_and_clear_persist() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        let id = {
            let mut history = store(dir.path(), HistoryPolicy::default(), now);
            let entry = history.add("delete me", None, now).unwrap().unwrap();
            assert!(history.delete(&entry.id).unwrap());
            assert!(!history.delete(&entry.id).unwrap());
            history.add("keep me", None, now).unwrap();
            entry.id
        };
        let mut history = store(dir.path(), HistoryPolicy::default(), now);
        assert_eq!(history.recent(10).len(), 1);
        assert!(!history.delete(&id).unwrap());
        history.clear().unwrap();
        drop(history);

        let history = store(dir.path(), HistoryPolicy::default(), now);
        assert!(history.entries().is_empty());
    }

    #[test]
    fn a_malformed_file_is_quarantined_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_700_000_000_000;
        let path = dir.path().join("history.json");
        fs::write(&path, b"{ not json").unwrap();

        let mut history = History::open(path.clone(), HistoryPolicy::default(), now).unwrap();
        assert!(history.recovered_from_corruption);
        assert!(history.entries().is_empty());
        history.add("fresh start", None, now).unwrap();

        let quarantined = dir.path().join(format!("history.corrupt-{now}.json"));
        assert!(quarantined.exists());
        assert!(path.exists());
    }

    #[test]
    fn ids_are_unique_within_a_millisecond() {
        let first = new_id(5);
        let second = new_id(5);
        assert_ne!(first, second);
    }

    #[test]
    fn a_missing_file_is_an_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let history = store(dir.path(), HistoryPolicy::default(), 0);
        assert!(history.entries().is_empty());
        assert!(!history.path().exists());
    }
}
