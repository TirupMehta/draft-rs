//! The engine worker. It owns the model for the life of the app, so the UI
//! thread never waits for a model load or a decode.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use draft_engine::{ComputeBackend, Engine, EngineConfig, EngineTimings, TranscriptionOptions};

pub struct Job {
    pub samples: Vec<f32>,
    pub serial: u64,
    /// A partial pass during capture: transcribed for display only, never
    /// pasted, and always superseded by the final job.
    pub preview: bool,
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
    },
    Transcribed {
        text: String,
        elapsed: Duration,
        timings: EngineTimings,
        audio_s: f64,
        paste_error: Option<String>,
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
    paste: bool,
) -> (Sender<Job>, Receiver<Event>) {
    let (job_sender, job_receiver) = channel();
    let (event_sender, event_receiver) = channel();

    std::thread::Builder::new()
        .name("draft-engine".to_owned())
        .spawn(move || run(model, backend, paste, job_receiver, event_sender))
        .expect("spawn the Draft engine thread");

    (job_sender, event_receiver)
}

fn run(
    model: PathBuf,
    backend: ComputeBackend,
    paste: bool,
    jobs: Receiver<Job>,
    events: Sender<Event>,
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

    while let Ok(job) = jobs.recv() {
        if job.preview {
            let text = match engine.transcribe(&job.samples, &TranscriptionOptions::default()) {
                Ok(result) => draft_core::text::clean_text(&result.text, false),
                Err(error) => {
                    tracing::debug!(%error, "preview pass failed");
                    String::new()
                }
            };
            if events
                .send(Event::Partial {
                    text,
                    serial: job.serial,
                })
                .is_err()
            {
                return;
            }
            continue;
        }

        let started = Instant::now();
        match engine.transcribe(&job.samples, &TranscriptionOptions::default()) {
            Ok(result) => {
                let cleaned = draft_core::text::clean_text(&result.text, false);
                let elapsed = started.elapsed();
                if cleaned.is_empty() || !draft_core::text::is_meaningful(&cleaned) {
                    let _ = events.send(Event::Empty { elapsed });
                    continue;
                }

                let paste_error = if paste {
                    draft_paste::paste_text(&cleaned)
                        .err()
                        .map(|error| format!("{error:#}"))
                } else {
                    None
                };
                let audio_s = job.samples.len() as f64 / 16_000.0;
                if events
                    .send(Event::Transcribed {
                        text: cleaned,
                        elapsed,
                        timings: result.timings,
                        audio_s,
                        paste_error,
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
