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
    pub learn_rhythm: bool,
}

impl Default for FormatJob {
    fn default() -> Self {
        Self {
            paragraphs: true,
            style: ParagraphStyle::Balanced,
            commands: true,
            learn_rhythm: true,
        }
    }
}

#[derive(Debug)]
pub enum Event {
    Ready {
        backend: String,
        device: String,
        load_ms: u128,
    },
    Failed(String),
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

pub fn spawn(
    model: PathBuf,
    backend: ComputeBackend,
) -> (Sender<Job>, Receiver<Event>, Arc<AtomicUsize>) {
    let (job_sender, job_receiver) = channel();
    let (event_sender, event_receiver) = channel();
    let pending_finals = Arc::new(AtomicUsize::new(0));
    let finals = pending_finals.clone();

    std::thread::Builder::new()
        .name("draft-engine".to_owned())
        .spawn(move || run(model, backend, job_receiver, event_sender, finals))
        .expect("spawn the Draft engine thread");

    (job_sender, event_receiver, pending_finals)
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
    model: PathBuf,
    backend: ComputeBackend,
    jobs: Receiver<Job>,
    events: Sender<Event>,
    pending_finals: Arc<AtomicUsize>,
) {
    if !model.exists() {
        let _ = events.send(Event::Failed(format!(
            "Model not found at {}. Download it with `draftctl model install`, or pass --model.",
            model.display()
        )));
        return;
    }

    let mut engine = match Engine::load(&EngineConfig::new(&model).backend(backend)) {
        Ok(engine) => engine,
        Err(error) => {
            let _ = events.send(Event::Failed(format!(
                "Could not load the model: {error:#}"
            )));
            return;
        }
    };

    let backend_name = engine.backend().to_owned();
    let device = engine.device().to_owned();
    let load_ms = engine.load_time().as_millis();
    let warmed = Instant::now();
    if let Err(error) = engine.warm_up() {
        let _ = events.send(Event::Failed(format!(
            "Could not warm the model: {error:#}"
        )));
        return;
    }
    let warm_ms = warmed.elapsed().as_millis();
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
        let result = engine.transcribe(&job.samples, &TranscriptionOptions::default());
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
        segments = segments.len(),
        "formatted the final transcript"
    );
    formatted.text
}
