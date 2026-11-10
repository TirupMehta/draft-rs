//! The engine worker. It owns the model for the life of the app, so the UI
//! thread never waits for a model load or a decode.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use draft_core::format::{self, ParagraphStyle};
use draft_core::rhythm::Rhythm;
use draft_engine::{ComputeBackend, Engine, EngineConfig, EngineTimings, TranscriptionOptions};

pub struct Job {
    pub samples: Vec<f32>,
    pub serial: u64,
    /// A partial pass during capture: transcribed for display only, never
    /// pasted, and always superseded by the final job.
    pub preview: bool,
    /// Whether this transcript is inserted at the cursor when it is ready.
    pub paste: bool,
    /// The formatting choices that apply to this job, copied from the settings
    /// when the key was released. Like `paste`, a switch takes effect on the
    /// next dictation without a restart.
    pub format: FormatJob,
}

/// One job's formatting choices.
#[derive(Debug, Clone)]
pub struct FormatJob {
    pub paragraphs: bool,
    pub style: ParagraphStyle,
    pub commands: bool,
    pub lists: bool,
    pub learn_rhythm: bool,
    /// Names and jargon to favour while recognizing. Context only; the
    /// formatter never rewrites finished text.
    pub vocabulary: String,
}

impl Default for FormatJob {
    fn default() -> Self {
        Self {
            paragraphs: true,
            style: ParagraphStyle::Balanced,
            commands: true,
            lists: false,
            learn_rhythm: true,
            vocabulary: String::new(),
        }
    }
}

/// The catalog entry that suits this machine, from the real hardware probe.
pub(crate) fn recommended_for_this_pc() -> &'static draft_core::models::ModelSpec {
    let hardware = draft_engine::detect_hardware();
    draft_core::models::recommend(&draft_core::models::Capability {
        discrete_gpu: hardware.discrete_gpu,
        integrated_gpu: hardware.integrated_gpu,
        vram_bytes: hardware.vram_bytes,
        ram_bytes: hardware.ram_bytes,
        cpu_threads: hardware.cpu_threads,
    })
}

#[derive(Debug)]
pub enum Event {
    Ready {
        backend: String,
        device: String,
        load_ms: u128,
    },
    Failed(String),
    /// The model this launch will use, so the app can remember the choice.
    Chosen {
        id: Option<&'static str>,
        label: String,
        downloading: bool,
    },
    /// Real download progress for the first-run model.
    Downloading {
        downloaded: u64,
        total: u64,
    },
    /// The GPU did not work; the engine is running on the CPU instead.
    FellBack {
        reason: String,
    },
    Partial {
        text: String,
        serial: u64,
        elapsed: Duration,
    },
    Transcribed {
        text: String,
        elapsed: Duration,
        timings: EngineTimings,
        audio_s: f64,
        /// True only when the text was actually inserted at the cursor.
        pasted: bool,
        paste_error: Option<String>,
        /// The pause profile after this clip, so the Formatting pane can show
        /// the speaker's real rhythm instead of a stored guess.
        rhythm: Rhythm,
    },
    Empty {
        elapsed: Duration,
    },
    JobFailed {
        error: String,
    },
}

/// Which model to run and how, decided before anything is loaded.
pub struct StartPlan {
    /// A path from `--model`, `DRAFT_MODEL_PATH`, or Settings. Used as is.
    pub explicit_model: Option<PathBuf>,
    /// The catalog id saved by an earlier launch.
    pub model_id: Option<String>,
    pub backend: ComputeBackend,
}

pub fn spawn(plan: StartPlan) -> (Sender<Job>, Receiver<Event>, Arc<AtomicUsize>) {
    let (job_sender, job_receiver) = channel();
    let (event_sender, event_receiver) = channel();
    let pending_finals = Arc::new(AtomicUsize::new(0));
    let finals = pending_finals.clone();

    std::thread::Builder::new()
        .name("draft-engine".to_owned())
        .spawn(move || run(plan, job_receiver, event_sender, finals))
        .expect("spawn the Draft engine thread");

    (job_sender, event_receiver, pending_finals)
}

/// Decide the model and make sure it is on disk, downloading it if needed.
/// Returns `None` after reporting a failure.
fn prepare_model(plan: &StartPlan, events: &Sender<Event>) -> Option<PathBuf> {
    use draft_core::models::{self, ModelState};

    if let Some(path) = &plan.explicit_model {
        if path.exists() {
            let _ = events.send(Event::Chosen {
                id: None,
                label: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                downloading: false,
            });
            return Some(path.clone());
        }
        let _ = events.send(Event::Failed(format!(
            "The model file {} is missing. Choose another model in Settings.",
            path.display()
        )));
        return None;
    }

    let dir = draft_core::paths::models_dir();
    let installed = |spec: &models::ModelSpec| {
        matches!(models::state(spec, &dir), Ok(ModelState::Installed { .. }))
    };

    let spec: &'static models::ModelSpec = match plan.model_id.as_deref().and_then(models::find) {
        Some(spec) => spec,
        None => {
            // A PC that already has a model keeps it; a fresh one is measured.
            if let Some(spec) = models::CATALOG.iter().find(|spec| installed(spec)) {
                spec
            } else {
                recommended_for_this_pc()
            }
        }
    };

    let present = installed(spec);
    let _ = events.send(Event::Chosen {
        id: Some(spec.id),
        label: format!("{} {}", spec.name, spec.quantization),
        downloading: !present,
    });
    if present {
        return Some(spec.path_in(&dir));
    }

    let source = models::HttpSource::new(spec);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut last_error = String::new();
    for attempt in 1..=4u64 {
        let mut last_sent = Instant::now() - Duration::from_secs(1);
        let result = models::install(spec, &dir, &source, &cancel, |progress| {
            if last_sent.elapsed() >= Duration::from_millis(200)
                || progress.downloaded >= progress.total
            {
                last_sent = Instant::now();
                let _ = events.send(Event::Downloading {
                    downloaded: progress.downloaded,
                    total: progress.total,
                });
            }
        });
        match result {
            Ok(path) => return Some(path),
            Err(error) => {
                last_error = format!("{error:#}");
                tracing::warn!(attempt, error = %last_error, "model download failed");
                std::thread::sleep(Duration::from_secs(2 * attempt));
            }
        }
    }
    let _ = events.send(Event::Failed(format!(
        "Draft could not download its speech model ({last_error}). \
         Check your internet connection and reopen Draft; the download resumes where it stopped."
    )));
    None
}

/// Retire one final job without ever wrapping the counter below zero. A
/// `compare_exchange` loop keeps this valid on the workspace MSRV.
fn finish_final(pending: &AtomicUsize) {
    let mut count = pending.load(Ordering::Acquire);
    while count > 0 {
        match pending.compare_exchange_weak(count, count - 1, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(actual) => count = actual,
        }
    }
}

fn run(
    plan: StartPlan,
    jobs: Receiver<Job>,
    events: Sender<Event>,
    pending_finals: Arc<AtomicUsize>,
) {
    let Some(model) = prepare_model(&plan, &events) else {
        return;
    };
    let backend = plan.backend;

    let mut engine = match Engine::load(&EngineConfig::new(&model).backend(backend)) {
        Ok(engine) => engine,
        Err(error) if backend != ComputeBackend::Cpu => {
            tracing::warn!(%error, "GPU load failed; retrying on the CPU");
            let _ = events.send(Event::FellBack {
                reason: format!("{error:#}"),
            });
            match Engine::load(&EngineConfig::new(&model).backend(ComputeBackend::Cpu)) {
                Ok(engine) => engine,
                Err(error) => {
                    let _ = events.send(Event::Failed(format!(
                        "Could not load the model: {error:#}"
                    )));
                    return;
                }
            }
        }
        Err(error) => {
            let _ = events.send(Event::Failed(format!(
                "Could not load the model: {error:#}"
            )));
            return;
        }
    };

    let mut backend_name = engine.backend().to_owned();
    let mut device = engine.device().to_owned();
    let load_ms = engine.load_time().as_millis();
    // The warm-up exists to compile GPU pipelines. On the CPU backend it is a
    // full silent encode that only delays the first dictation, so skip it and
    // report ready immediately.
    let warm_ms = if backend_name.eq_ignore_ascii_case("cpu") {
        tracing::info!("skipping warm-up on the CPU backend");
        0
    } else {
        let warmed = Instant::now();
        if let Err(error) = engine.warm_up() {
            tracing::warn!(%error, "GPU warm-up failed; retrying on the CPU");
            let _ = events.send(Event::FellBack {
                reason: format!("{error:#}"),
            });
            match Engine::load(&EngineConfig::new(&model).backend(ComputeBackend::Cpu)) {
                Ok(cpu) => engine = cpu,
                Err(error) => {
                    let _ = events.send(Event::Failed(format!(
                        "Could not load the model: {error:#}"
                    )));
                    return;
                }
            }
        }
        backend_name = engine.backend().to_owned();
        device = engine.device().to_owned();
        warmed.elapsed().as_millis()
    };
    tracing::info!(
        backend = %backend_name,
        device = %device,
        load_ms,
        warm_ms,
        "engine ready"
    );

    if events
        .send(Event::Ready {
            backend: backend_name,
            device,
            load_ms,
        })
        .is_err()
    {
        return;
    }

    // Once one final pass has identified the language, previews can skip
    // auto-detection and decode with that hint. The final pass still detects
    // the language itself, so switching languages remains supported.
    let mut language: Option<String> = None;

    // The learned pause profile. It lives with the worker because that is where
    // a clip is measured, and it is only ever read or written on this thread.
    let rhythm_path = Rhythm::default_path();
    let mut rhythm = Rhythm::load_or_default(&rhythm_path);

    while let Ok(job) = jobs.recv() {
        if job.preview {
            // A final result always goes first. If one is already waiting,
            // this preview has been superseded and must not delay it.
            if pending_finals.load(Ordering::Acquire) > 0 {
                continue;
            }
            let started = Instant::now();
            let text = match engine.transcribe(
                &job.samples,
                &TranscriptionOptions {
                    language: language.clone(),
                    low_latency: true,
                    ..TranscriptionOptions::default()
                },
            ) {
                Ok(result) => draft_core::text::clean_text(&result.text, false),
                Err(error) => {
                    tracing::debug!(%error, "preview pass failed");
                    String::new()
                }
            };
            let elapsed = started.elapsed();
            tracing::debug!(
                elapsed_ms = elapsed.as_millis(),
                audio_s = job.samples.len() as f64 / 16_000.0,
                "preview pass"
            );
            if events
                .send(Event::Partial {
                    text,
                    serial: job.serial,
                    elapsed,
                })
                .is_err()
            {
                return;
            }
            continue;
        }

        let started = Instant::now();
        if is_silence(&job.samples) {
            // Whisper invents "Thank you." on silence. Do not ask it.
            finish_final(&pending_finals);
            let _ = events.send(Event::Empty {
                elapsed: started.elapsed(),
            });
            continue;
        }
        let vocabulary = job.format.vocabulary.trim();
        let result = engine.transcribe(
            &job.samples,
            &TranscriptionOptions {
                vocabulary: vocabulary
                    .split([',', '\n'])
                    .map(|word| word.trim().to_owned())
                    .filter(|word| !word.is_empty())
                    .collect(),
                ..TranscriptionOptions::default()
            },
        );
        finish_final(&pending_finals);
        match result {
            Ok(result) => {
                let cleaned = draft_core::text::clean_text(&result.text, false);
                if let Some(detected) = result.language.clone() {
                    language = Some(detected);
                }
                let elapsed = started.elapsed();
                if cleaned.is_empty() || !draft_core::text::is_meaningful(&cleaned) {
                    let _ = events.send(Event::Empty { elapsed });
                    continue;
                }

                let text = format_final(&cleaned, &result, &job.format, &mut rhythm, &rhythm_path);
                if text.is_empty() || !draft_core::text::is_meaningful(&text) {
                    // Everything was scratched, or the rules produced nothing.
                    let _ = events.send(Event::Empty { elapsed });
                    continue;
                }

                let paste_error = if job.paste {
                    draft_paste::paste_text(&draft_core::text::for_paste(&text))
                        .err()
                        .map(|error| format!("{error:#}"))
                } else {
                    None
                };
                let pasted = job.paste && paste_error.is_none();
                let audio_s = job.samples.len() as f64 / 16_000.0;
                if events
                    .send(Event::Transcribed {
                        text,
                        elapsed,
                        timings: result.timings,
                        audio_s,
                        pasted,
                        paste_error,
                        rhythm,
                    })
                    .is_err()
                {
                    return;
                }
            }
            Err(error) => {
                let _ = events.send(Event::JobFailed {
                    error: format!("{error:#}"),
                });
            }
        }
    }
}

/// Apply the formatting rules to a cleaned final transcript and fold the clip's
/// pause statistics into the profile.
///
/// Formatting runs once, on the final text, after cleanup and before paste. The
/// live preview stays plain: it is a different, cheaper decode of a partial
/// clip, and formatting a partial would only make it flicker.
fn format_final(
    cleaned: &str,
    result: &draft_engine::Transcription,
    job: &FormatJob,
    rhythm: &mut Rhythm,
    rhythm_path: &std::path::Path,
) -> String {
    let segments = result
        .segments
        .iter()
        .map(|segment| format::Segment::new(segment.text.clone(), segment.start_ms, segment.end_ms))
        .collect::<Vec<_>>();
    let options = format::Options {
        paragraphs: job.paragraphs,
        style: job.style,
        commands: job.commands,
        lists: job.lists,
        language: result.language.clone(),
    };

    let formatted = format::format(cleaned, &segments, &options, rhythm);
    if let Some(observed) = formatted.observed.filter(|_| job.learn_rhythm) {
        rhythm.observe(observed.median_gap_ms, observed.p90_gap_ms);
        if let Err(error) = rhythm.save(rhythm_path) {
            tracing::warn!(%error, "could not save the pause profile");
        }
    }
    tracing::debug!(
        breaks = formatted.breaks,
        commands = formatted.commands,
        items = formatted.items,
        segments = segments.len(),
        "formatted the final transcript"
    );
    formatted.text
}

/// True when a clip has no speech-level signal at all: a key tapped in a quiet
/// room, a muted microphone. The threshold is far below any real voice, so it
/// never drops quiet speech, only a flat line.
fn is_silence(samples: &[f32]) -> bool {
    if samples.is_empty() {
        return true;
    }
    let peak = samples.iter().fold(0.0_f32, |acc, s| acc.max(s.abs()));
    peak < 0.004
}
