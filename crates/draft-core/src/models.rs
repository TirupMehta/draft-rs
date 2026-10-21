//! The local model catalog and its resumable, checksum-verified installer.
//!
//! One model is one GGUF file. The catalog pins the exact artifact that was
//! benchmarked - byte length and SHA-256 - so an install either produces those
//! bytes or fails loudly. Nothing is trusted because its file name looks right.
//!
//! The installer writes to `<name>.part`, resumes from whatever prefix is
//! already on disk, verifies the complete file, syncs it, and only then renames
//! it into place. An interrupted download leaves a resumable prefix and never a
//! half-written model that the engine could load.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

/// One pinned GGUF artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSpec {
    /// Stable identifier used on the command line and in settings.
    pub id: &'static str,
    /// Product-facing name.
    pub name: &'static str,
    /// Quantization label, for example `Q8_0`.
    pub quantization: &'static str,
    /// File name inside the models folder.
    pub file_name: &'static str,
    /// The exact download URL.
    pub url: &'static str,
    /// Exact length in bytes.
    pub size: u64,
    /// Lowercase hex SHA-256 of the exact bytes.
    pub sha256: &'static str,
}

/// Whisper large-v3-turbo at float16, the same model and precision as Draft
/// v3.5 (`large-v3-turbo` with `compute_type="float16"` on CUDA).
///
/// The checksum was computed from the downloaded artifact, so the shipped file
/// is provably the one that was measured.
pub const WHISPER_LARGE_V3_TURBO_F16: ModelSpec = ModelSpec {
    id: "whisper-large-v3-turbo-f16",
    name: "Whisper Large v3 Turbo",
    quantization: "F16",
    file_name: "whisper-large-v3-turbo-F16.gguf",
    url: "https://huggingface.co/handy-computer/whisper-large-v3-turbo-gguf/resolve/main/whisper-large-v3-turbo-F16.gguf",
    size: 1_625_935_488,
    sha256: "c8506ddfa3d4eb9279170d722a75422db00dbdbd7618531bbb81926e0598eb8f",
};

/// The same weights at 8-bit, about half the size and a little faster.
///
/// Kept as an explicit catalog entry for constrained machines. It is never the
/// default because 8-bit weights are not the precision v3.5 ships.
pub const WHISPER_LARGE_V3_TURBO_Q8_0: ModelSpec = ModelSpec {
    id: "whisper-large-v3-turbo-q8_0",
    name: "Whisper Large v3 Turbo",
    quantization: "Q8_0",
    file_name: "whisper-large-v3-turbo-Q8_0.gguf",
    url: "https://huggingface.co/handy-computer/whisper-large-v3-turbo-gguf/resolve/main/whisper-large-v3-turbo-Q8_0.gguf",
    size: 886_381_728,
    sha256: "257aa7ec9121c22c167d5e2c49e3e76b4ba0e6e27ebc13074eba0a80f8190220",
};

/// The artifact a fresh install expects.
pub const DEFAULT_MODEL: ModelSpec = WHISPER_LARGE_V3_TURBO_F16;

/// Everything a fresh install can fetch.
pub const CATALOG: &[ModelSpec] = &[WHISPER_LARGE_V3_TURBO_F16, WHISPER_LARGE_V3_TURBO_Q8_0];

/// Look up a catalog entry by id.
pub fn find(id: &str) -> Option<&'static ModelSpec> {
    CATALOG.iter().find(|model| model.id == id)
}

/// Bytes read or written per I/O call.
const CHUNK: usize = 1 << 20;

impl ModelSpec {
    /// The installed path inside `dir`.
    pub fn path_in(&self, dir: &Path) -> PathBuf {
        dir.join(self.file_name)
    }

    /// The resumable partial download inside `dir`.
    pub fn partial_path_in(&self, dir: &Path) -> PathBuf {
        dir.join(format!("{}.part", self.file_name))
    }
}

/// What is on disk right now, judged by presence and length only.
///
/// Length is cheap and catches interrupted copies. [`verify`] is the authority
/// on content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelState {
    Missing,
    Partial {
        bytes: u64,
    },
    Installed {
        bytes: u64,
    },
    /// A complete-length file that still failed verification when checked, or
    /// a file with the wrong length.
    Corrupt {
        bytes: u64,
    },
}

impl ModelState {
    pub fn is_installed(self) -> bool {
        matches!(self, Self::Installed { .. })
    }
}

/// Inspect `dir` for `spec` without hashing the file.
pub fn state(spec: &ModelSpec, dir: &Path) -> Result<ModelState> {
    let path = spec.path_in(dir);
    match fs::metadata(&path) {
        Ok(metadata) => {
            let bytes = metadata.len();
            Ok(if bytes == spec.size {
                ModelState::Installed { bytes }
            } else {
                ModelState::Corrupt { bytes }
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match fs::metadata(spec.partial_path_in(dir)) {
                Ok(metadata) => Ok(ModelState::Partial {
                    bytes: metadata.len(),
                }),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(ModelState::Missing)
                }
                Err(error) => Err(error).context("inspect the partial model"),
            }
        }
        Err(error) => Err(error).context("inspect the installed model"),
    }
}

/// The result of hashing a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub bytes: u64,
    pub sha256: String,
}

impl Verification {
    /// Whether these bytes are exactly the pinned artifact.
    pub fn matches(&self, spec: &ModelSpec) -> bool {
        self.bytes == spec.size && self.sha256.eq_ignore_ascii_case(spec.sha256)
    }
}

/// Hash `path` and report its length.
pub fn verify(path: &Path, spec: &ModelSpec) -> Result<Verification> {
    let mut file =
        File::open(path).with_context(|| format!("open {} for verification", path.display()))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        if bytes > spec.size {
            // Keep hashing past the expected length so the report is honest,
            // but stop at a bounded overrun.
            if bytes > spec.size.saturating_add(64 * 1024 * 1024) {
                bail!("{} is far larger than the pinned artifact", path.display());
            }
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Verification {
        bytes,
        sha256: hex(&hasher.finalize()),
    })
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Progress for one install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub downloaded: u64,
    pub total: u64,
}

/// Where the bytes come from. Tests substitute an in-memory source.
pub trait ByteSource {
    /// Expected remaining-and-prefix length of the whole artifact.
    fn total_size(&self) -> u64;

    /// Read the artifact from `offset` to its end. Implementations must start
    /// exactly at `offset`; returning bytes from the beginning would corrupt
    /// the resumed file.
    fn open_from(&self, offset: u64) -> Result<Box<dyn Read + Send>>;
}

/// The real, resumable HTTPS source.
pub struct HttpSource {
    url: &'static str,
    size: u64,
}

impl HttpSource {
    pub fn new(spec: &'static ModelSpec) -> Self {
        Self {
            url: spec.url,
            size: spec.size,
        }
    }
}

impl ByteSource for HttpSource {
    fn total_size(&self) -> u64 {
        self.size
    }

    fn open_from(&self, offset: u64) -> Result<Box<dyn Read + Send>> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(30))
            .build();
        let request = agent.get(self.url);
        let response = if offset == 0 {
            request.call()
        } else {
            request.set("Range", &format!("bytes={offset}-")).call()
        }
        .with_context(|| format!("download {}", self.url))?;
        if offset > 0 && response.status() != 206 {
            bail!(
                "the server ignored the resume request (HTTP {})",
                response.status()
            );
        }
        Ok(Box::new(response.into_reader()))
    }
}

/// Why an install stopped.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("download cancelled")]
    Cancelled,
    #[error("the download ended early: expected {expected} bytes, got {actual}")]
    Short { expected: u64, actual: u64 },
    #[error(
        "the server sent more than the pinned artifact: expected {expected} bytes, got {actual}"
    )]
    Long { expected: u64, actual: u64 },
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    Checksum { expected: String, actual: String },
}

/// Download, verify, and atomically install `spec` into `dir`.
///
/// A correct file that is already present is returned untouched. An incorrect
/// one is removed; a partial download resumes. Cancellation keeps the partial
/// file so the next call continues where it stopped.
pub fn install<F>(
    spec: &ModelSpec,
    dir: &Path,
    source: &dyn ByteSource,
    cancel: &AtomicBool,
    mut on_progress: F,
) -> Result<PathBuf>
where
    F: FnMut(Progress),
{
    if source.total_size() != spec.size {
        bail!(
            "source reports {} bytes but the pinned artifact is {}",
            source.total_size(),
            spec.size
        );
    }

    fs::create_dir_all(dir)
        .with_context(|| format!("create the models folder {}", dir.display()))?;

    let final_path = spec.path_in(dir);
    if final_path.exists() {
        let existing = verify(&final_path, spec)?;
        if existing.matches(spec) {
            on_progress(Progress {
                downloaded: spec.size,
                total: spec.size,
            });
            return Ok(final_path);
        }
        tracing::warn!(
            path = %final_path.display(),
            bytes = existing.bytes,
            "removing a model file that does not match the pinned artifact"
        );
        fs::remove_file(&final_path)
            .with_context(|| format!("remove the invalid model {}", final_path.display()))?;
    }

    let partial_path = spec.partial_path_in(dir);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&partial_path)
        .with_context(|| format!("open the partial download {}", partial_path.display()))?;

    let mut written = file
        .metadata()
        .context("inspect the partial download")?
        .len();
    if written > spec.size {
        tracing::warn!(
            path = %partial_path.display(),
            bytes = written,
            "discarding a partial download longer than the artifact"
        );
        file.set_len(0).context("truncate the partial download")?;
        written = 0;
    }
    file.seek(SeekFrom::End(0))
        .context("seek to the end of the partial download")?;

    // Hash the prefix that survived the last attempt so a resumed download is
    // verified against the whole file, not just the new tail.
    let mut hasher = Sha256::new();
    if written > 0 {
        let mut prefix = File::open(&partial_path).context("reopen the partial download")?;
        let mut buffer = vec![0u8; CHUNK];
        let mut hashed = 0u64;
        loop {
            let read = prefix
                .read(&mut buffer)
                .context("hash the partial download")?;
            if read == 0 {
                break;
            }
            hashed += read as u64;
            hasher.update(&buffer[..read]);
        }
        if hashed != written {
            bail!("the partial download changed while it was being read");
        }
    }

    on_progress(Progress {
        downloaded: written,
        total: spec.size,
    });

    if written < spec.size {
        let mut reader = source.open_from(written)?;
        let mut buffer = vec![0u8; CHUNK];
        loop {
            if cancel.load(Ordering::Relaxed) {
                file.flush().context("flush the partial download")?;
                file.sync_all().context("sync the partial download")?;
                return Err(InstallError::Cancelled.into());
            }
            let read = reader.read(&mut buffer).context("read the download")?;
            if read == 0 {
                break;
            }
            if written + read as u64 > spec.size {
                bail!(InstallError::Long {
                    expected: spec.size,
                    actual: written + read as u64,
                });
            }
            file.write_all(&buffer[..read])
                .context("write the partial download")?;
            hasher.update(&buffer[..read]);
            written += read as u64;
            on_progress(Progress {
                downloaded: written,
                total: spec.size,
            });
        }
    }

    file.flush().context("flush the partial download")?;
    file.sync_all().context("sync the partial download")?;
    drop(file);

    if written != spec.size {
        bail!(InstallError::Short {
            expected: spec.size,
            actual: written,
        });
    }

    let actual = hex(&hasher.finalize());
    if !actual.eq_ignore_ascii_case(spec.sha256) {
        let _ = fs::remove_file(&partial_path);
        bail!(InstallError::Checksum {
            expected: spec.sha256.to_owned(),
            actual,
        });
    }

    fs::rename(&partial_path, &final_path).with_context(|| {
        format!(
            "move the verified model into place at {}",
            final_path.display()
        )
    })?;
    Ok(final_path)
}

/// Delete the installed file and any partial download.
pub fn remove(spec: &ModelSpec, dir: &Path) -> Result<bool> {
    let mut removed = false;
    for path in [spec.path_in(dir), spec.partial_path_in(dir)] {
        match fs::remove_file(&path) {
            Ok(()) => removed = true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("remove {}", path.display()));
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::Mutex;

    use super::*;

    const BYTES: &[u8] = b"the quick brown fox jumps over the lazy dog";

    fn spec_for(bytes: &[u8]) -> ModelSpec {
        ModelSpec {
            id: "test",
            name: "Test",
            quantization: "F32",
            file_name: "test.gguf",
            url: "https://example.invalid/test.gguf",
            size: bytes.len() as u64,
            sha256: Box::leak(hex(&Sha256::digest(bytes)).into_boxed_str()),
        }
    }

    struct FakeSource {
        bytes: Vec<u8>,
        offsets: Mutex<Vec<u64>>,
    }

    impl FakeSource {
        fn new(bytes: &[u8]) -> Self {
            Self {
                bytes: bytes.to_vec(),
                offsets: Mutex::new(Vec::new()),
            }
        }

        fn offsets(&self) -> Vec<u64> {
            self.offsets.lock().unwrap().clone()
        }
    }

    impl ByteSource for FakeSource {
        fn total_size(&self) -> u64 {
            self.bytes.len() as u64
        }

        fn open_from(&self, offset: u64) -> Result<Box<dyn Read + Send>> {
            self.offsets.lock().unwrap().push(offset);
            Ok(Box::new(Cursor::new(
                self.bytes[offset as usize..].to_vec(),
            )))
        }
    }

    #[test]
    fn the_catalog_pins_a_well_formed_artifact() {
        assert!(!CATALOG.is_empty());
        for model in CATALOG {
            assert_eq!(model.sha256.len(), 64, "{} needs a full SHA-256", model.id);
            assert!(model.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(model.size > 0);
            assert!(model.file_name.ends_with(".gguf"));
            assert!(model.url.starts_with("https://"));
            assert!(model.file_name.starts_with("whisper-large-v3-turbo"));
        }
        assert!(find(DEFAULT_MODEL.id).is_some());
    }

    #[test]
    fn verification_reports_the_exact_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.gguf");
        fs::write(&path, BYTES).unwrap();

        let spec = spec_for(BYTES);
        let report = verify(&path, &spec).unwrap();
        assert!(report.matches(&spec));
        assert_eq!(report.bytes, BYTES.len() as u64);

        let other = spec_for(b"different bytes");
        assert!(!report.matches(&other));
    }

    #[test]
    fn install_writes_a_verified_file_and_leaves_no_partial() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        let source = FakeSource::new(BYTES);
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();

        let path = install(&spec, dir.path(), &source, &cancel, |progress| {
            seen.push(progress)
        })
        .unwrap();

        assert_eq!(fs::read(&path).unwrap(), BYTES);
        assert!(!spec.partial_path_in(dir.path()).exists());
        assert_eq!(source.offsets(), vec![0]);
        assert_eq!(
            state(&spec, dir.path()).unwrap(),
            ModelState::Installed {
                bytes: BYTES.len() as u64
            }
        );
        assert_eq!(seen.first().unwrap().downloaded, 0);
        assert_eq!(seen.last().unwrap().downloaded, BYTES.len() as u64);
        assert!(
            seen.windows(2)
                .all(|pair| pair[0].downloaded <= pair[1].downloaded)
        );
    }

    #[test]
    fn install_returns_an_already_correct_file_without_downloading() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        fs::write(spec.path_in(dir.path()), BYTES).unwrap();

        let source = FakeSource::new(BYTES);
        let cancel = AtomicBool::new(false);
        let path = install(&spec, dir.path(), &source, &cancel, |_| {}).unwrap();

        assert_eq!(fs::read(&path).unwrap(), BYTES);
        assert!(source.offsets().is_empty(), "should not have downloaded");
    }

    #[test]
    fn install_resumes_from_the_partial_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        let split = 17;
        fs::write(spec.partial_path_in(dir.path()), &BYTES[..split]).unwrap();

        let source = FakeSource::new(BYTES);
        let cancel = AtomicBool::new(false);
        let path = install(&spec, dir.path(), &source, &cancel, |_| {}).unwrap();

        assert_eq!(fs::read(&path).unwrap(), BYTES);
        assert_eq!(source.offsets(), vec![split as u64]);
    }

    #[test]
    fn install_resumes_a_complete_partial_without_downloading() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        fs::write(spec.partial_path_in(dir.path()), BYTES).unwrap();

        let source = FakeSource::new(BYTES);
        let cancel = AtomicBool::new(false);
        let path = install(&spec, dir.path(), &source, &cancel, |_| {}).unwrap();

        assert_eq!(fs::read(&path).unwrap(), BYTES);
        assert!(source.offsets().is_empty());
    }

    #[test]
    fn install_replaces_a_corrupt_installed_file() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        fs::write(spec.path_in(dir.path()), b"corrupt").unwrap();

        let source = FakeSource::new(BYTES);
        let cancel = AtomicBool::new(false);
        let path = install(&spec, dir.path(), &source, &cancel, |_| {}).unwrap();

        assert_eq!(fs::read(&path).unwrap(), BYTES);
        assert_eq!(source.offsets(), vec![0]);
    }

    #[test]
    fn install_rejects_a_checksum_mismatch_and_keeps_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        let source = FakeSource::new(b"the quick brown fox jumps over the lazy d0g");
        let cancel = AtomicBool::new(false);

        let error = install(&spec, dir.path(), &source, &cancel, |_| {}).unwrap_err();
        assert!(
            error.downcast_ref::<InstallError>().is_some(),
            "unexpected error: {error:#}"
        );
        assert!(matches!(
            error.downcast_ref::<InstallError>(),
            Some(InstallError::Checksum { .. })
        ));
        assert!(!spec.path_in(dir.path()).exists());
        assert!(!spec.partial_path_in(dir.path()).exists());
    }

    #[test]
    fn install_stops_on_cancellation_and_keeps_the_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        let source = FakeSource::new(BYTES);
        let cancel = AtomicBool::new(false);

        let error = install(&spec, dir.path(), &source, &cancel, |_| {
            cancel.store(true, Ordering::Relaxed);
        })
        .unwrap_err();

        assert!(matches!(
            error.downcast_ref::<InstallError>(),
            Some(InstallError::Cancelled)
        ));
        assert!(!spec.path_in(dir.path()).exists());
        assert!(spec.partial_path_in(dir.path()).exists());
    }

    #[test]
    fn install_rejects_a_source_of_the_wrong_length() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        let source = FakeSource::new(b"short");
        let cancel = AtomicBool::new(false);

        let error = install(&spec, dir.path(), &source, &cancel, |_| {}).unwrap_err();
        assert!(error.to_string().contains("source reports"));
    }

    #[test]
    fn install_discards_a_partial_longer_than_the_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        fs::write(spec.partial_path_in(dir.path()), vec![0u8; BYTES.len() + 5]).unwrap();

        let source = FakeSource::new(BYTES);
        let cancel = AtomicBool::new(false);
        let path = install(&spec, dir.path(), &source, &cancel, |_| {}).unwrap();

        assert_eq!(fs::read(&path).unwrap(), BYTES);
        assert_eq!(source.offsets(), vec![0]);
    }

    #[test]
    fn state_separates_missing_partial_and_installed() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);

        assert_eq!(state(&spec, dir.path()).unwrap(), ModelState::Missing);

        fs::write(spec.partial_path_in(dir.path()), b"half").unwrap();
        assert_eq!(
            state(&spec, dir.path()).unwrap(),
            ModelState::Partial { bytes: 4 }
        );

        fs::write(spec.path_in(dir.path()), &BYTES[..5]).unwrap();
        assert_eq!(
            state(&spec, dir.path()).unwrap(),
            ModelState::Corrupt { bytes: 5 }
        );
    }

    #[test]
    fn remove_deletes_the_file_and_the_partial() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for(BYTES);
        fs::write(spec.path_in(dir.path()), BYTES).unwrap();
        fs::write(spec.partial_path_in(dir.path()), b"part").unwrap();

        assert!(remove(&spec, dir.path()).unwrap());
        assert!(!remove(&spec, dir.path()).unwrap());
        assert_eq!(state(&spec, dir.path()).unwrap(), ModelState::Missing);
    }
}
