//! The warm local speech engine.
//!
//! One model stays loaded for the life of the app. Every call feeds 16 kHz
//! mono PCM to the same session, so the GPU pipelines, weights, and decoder
//! state are already warm when the user releases the hotkey.
//!
//! The engine deliberately owns no microphone, file format, or UI concern.
//! Shells turn audio into `&[f32]` and call [`Engine::transcribe`].

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use thiserror::Error;
use tracing::{debug, info, warn};
use transcribe_cpp::{
    Backend, Model, ModelOptions, RunExtension, RunOptions, Session, WhisperRunOptions,
};

/// Everything that can stop the engine from producing text.
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Native(#[from] transcribe_cpp::Error),
}

/// Which compute backend to request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComputeBackend {
    /// Let the library pick the best available device.
    #[default]
    Auto,
    Vulkan,
    Metal,
    Cuda,
    Cpu,
}

impl ComputeBackend {
    fn native(self) -> Backend {
        match self {
            Self::Auto => Backend::Auto,
            Self::Vulkan => Backend::Vulkan,
            Self::Metal => Backend::Metal,
            Self::Cuda => Backend::Cuda,
            Self::Cpu => Backend::Cpu,
        }
    }
}

/// How to load the model.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub model_path: PathBuf,
    pub backend: ComputeBackend,
}

impl EngineConfig {
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            backend: ComputeBackend::Auto,
        }
    }

    pub fn backend(mut self, backend: ComputeBackend) -> Self {
        self.backend = backend;
        self
    }
}

/// Per-call transcription hints.
#[derive(Debug, Clone, Default)]
pub struct TranscriptionOptions {
    /// ISO language code, or `None` to detect.
    pub language: Option<String>,
    /// Names and product terms to bias toward.
    pub vocabulary: Vec<String>,
    /// Free-form context that helps names and punctuation.
    pub prompt: Option<String>,
    /// Use the cheapest decode path for a display-only partial. This disables
    /// temperature fallback and previous-token conditioning, so a preview is
    /// one pass instead of a possible retry loop. Final text never uses it.
    pub low_latency: bool,
}

/// Stage timings reported by the native engine.
#[derive(Debug, Clone, Copy, Default)]
pub struct EngineTimings {
    pub mel_ms: f32,
    pub encode_ms: f32,
    pub decode_ms: f32,
}

impl EngineTimings {
    pub fn total_ms(self) -> f32 {
        self.mel_ms + self.encode_ms + self.decode_ms
    }
}

/// One timed span of a finished transcript.
///
/// Segment timings come from the model's own decoder rows and cost nothing
/// extra: the native library already produces them for every run. They are the
/// only honest signal a shell has for where a speaker paused, so they are
/// carried through rather than discarded. `start_ms`/`end_ms` are measured from
/// the first sample of the clip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Segment {
    /// The segment text, trimmed but otherwise exactly as decoded.
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

impl Segment {
    /// Silence between this segment and the one before it, clamped at zero.
    ///
    /// A negative value would mean overlapping rows; treating that as "no
    /// pause" is the only safe reading, because a formatting rule must never
    /// break text on a measurement the decoder did not make.
    pub fn gap_before(&self, previous: &Segment) -> i64 {
        (self.start_ms - previous.end_ms).max(0)
    }
}

/// A finished transcript.
#[derive(Debug, Clone, Default)]
pub struct Transcription {
    pub text: String,
    pub language: Option<String>,
    pub timings: EngineTimings,
    /// Timed rows, in order, with empty rows removed. May be empty when the
    /// backend does not report segment timings; callers must treat that as
    /// "unknown" rather than "no pauses".
    pub segments: Vec<Segment>,
    /// Wall time from entering the engine to the finished transcript.
    pub elapsed: Duration,
}

/// A loaded model and its single reusable session.
///
/// The native library serializes compute per model, so one session per app is
/// the intended shape. `Session` is `Send`, which lets the desktop shell own the
/// engine on a worker thread while the UI stays responsive.
pub struct Engine {
    // Kept alive alongside the session so the native model cannot be freed
    // before the last derived session.
    _model: Model,
    session: Session,
    model_path: PathBuf,
    arch: String,
    backend: String,
    device: String,
    load_time: Duration,
}

impl Engine {
    /// Load a GGUF model and prepare a session. This does not run inference;
    /// call [`Engine::warm_up`] at app launch to compile GPU pipelines.
    pub fn load(config: &EngineConfig) -> Result<Self, Error> {
        let started = Instant::now();
        let options = ModelOptions {
            backend: config.backend.native(),
            device: None,
        };
        let model = Model::load_with(&config.model_path, &options)?;
        let load_time = started.elapsed();
        let backend = model.backend();
        let arch = model.arch();
        let device = model
            .device()
            .map(|device| device.name)
            .unwrap_or_else(|_| "unknown".to_owned());
        let session = model.session()?;

        info!(
            model = %config.model_path.display(),
            arch = %arch,
            backend = %backend,
            device = %device,
            load_ms = load_time.as_secs_f64() * 1000.0,
            "speech engine loaded"
        );

        Ok(Self {
            _model: model,
            session,
            model_path: config.model_path.clone(),
            arch,
            backend,
            device,
            load_time,
        })
    }

    /// Run one short silent pass so the backend compiles its kernels and
    /// allocates its buffers before the first real dictation.
    ///
    /// Decode errors on silence are expected and harmless; a backend failure is
    /// reported. The call deliberately returns `Ok` once compute ran, because a
    /// silent warm-up must not block a working app.
    pub fn warm_up(&mut self) -> Result<(), Error> {
        let started = Instant::now();
        let silence = vec![0.0_f32; 16_000];
        match self.transcribe(&silence, &TranscriptionOptions::default()) {
            Ok(_) => {}
            Err(error) => {
                warn!(%error, "warm-up decode did not produce text");
            }
        }
        info!(
            warm_ms = started.elapsed().as_secs_f64() * 1000.0,
            "speech engine warmed"
        );
        Ok(())
    }

    /// Transcribe 16 kHz mono PCM in `[-1, 1]`.
    pub fn transcribe(
        &mut self,
        pcm: &[f32],
        options: &TranscriptionOptions,
    ) -> Result<Transcription, Error> {
        if pcm.is_empty() {
            return Ok(Transcription::default());
        }

        let started = Instant::now();
        let family = if options.low_latency && self.arch == "whisper" {
            Some(RunExtension::Whisper(WhisperRunOptions {
                condition_on_prev_tokens: Some(false),
                temperature: Some(0.0),
                // Zero disables whisper.cpp's temperature fallback loop.
                temperature_inc: Some(0.0),
                max_prev_context_tokens: Some(0),
                ..WhisperRunOptions::default()
            }))
        } else {
            None
        };
        let native = RunOptions {
            language: options.language.clone(),
            vocabulary: options.vocabulary.clone(),
            prompt: options.prompt.clone(),
            family,
            ..RunOptions::default()
        };
        let transcript = self.session.run(pcm, &native)?;
        let elapsed = started.elapsed();
        let timings = EngineTimings {
            mel_ms: transcript.timings.mel_ms,
            encode_ms: transcript.timings.encode_ms,
            decode_ms: transcript.timings.decode_ms,
        };
        // Rows with no text carry no boundary, and a row that starts before the
        // previous one ends cannot describe a pause. Keep the rows that can be
        // read and drop the rest, so a caller never has to guard against them.
        let segments = transcript
            .segments
            .iter()
            .filter(|row| !row.text.trim().is_empty())
            .map(|row| Segment {
                text: row.text.trim().to_owned(),
                start_ms: row.t0_ms,
                end_ms: row.t1_ms,
            })
            .collect::<Vec<_>>();

        debug!(
            elapsed_ms = elapsed.as_secs_f64() * 1000.0,
            mel_ms = timings.mel_ms,
            encode_ms = timings.encode_ms,
            decode_ms = timings.decode_ms,
            chars = transcript.text.len(),
            "transcribed"
        );

        Ok(Transcription {
            text: transcript.text,
            language: transcript.language,
            timings,
            segments,
            elapsed,
        })
    }

    /// The backend actually in use, e.g. `vulkan` or `cpu`.
    pub fn backend(&self) -> &str {
        &self.backend
    }

    /// The compute device name reported by the backend.
    pub fn device(&self) -> &str {
        &self.device
    }

    pub fn load_time(&self) -> Duration {
        self.load_time
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }
}
