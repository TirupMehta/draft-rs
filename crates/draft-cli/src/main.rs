use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use draft_core::format::{self, ParagraphStyle};
use draft_core::rhythm::Rhythm;
use draft_core::text;
use draft_engine::{
    ComputeBackend, Engine, EngineConfig, EngineTimings, Segment, TranscriptionOptions,
};
use draft_hotkey::{HotkeyConfig, HotkeyEvent};
use serde::Serialize;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SAMPLE_RATE: u32 = 16_000;

fn main() {
    if let Err(error) = run() {
        eprintln!("draft: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        None | Some("app") => {
            println!("Draft {VERSION}");
            println!("The native shell lives in apps/desktop. Run `cargo run -p draft-desktop`.");
        }
        Some("clean") => {
            let input = args[1..].join(" ");
            println!("{}", text::clean_text(&input, false));
        }
        Some("transcribe") => transcribe(&args[1..])?,
        Some("bench") => bench(&args[1..])?,
        Some("mic") => mic(&args[1..])?,
        Some("listen") => listen(&args[1..])?,
        Some("model") => model(&args[1..])?,
        Some("history") => history(&args[1..])?,
        Some("--version" | "-V") => println!("draft {VERSION}"),
        Some("--help" | "-h") => print_help(),
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_help();
            std::process::exit(2);
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        "Draft {VERSION}

Usage:
  draft                         Open the app (native shell is separate)
  draft clean <text>            Print the cleaned transcript
  draft transcribe <file.wav>   Transcribe a 16 kHz mono WAV
  draft bench --fixtures <dir>  Measure warm latency and write JSON
  draft mic --seconds <n>       Record the default microphone
  draft listen --model <path>   Hold Right Ctrl to dictate into the terminal
  draft model status            Show the installed model and its path
  draft model list              Show the model catalog
  draft model install           Download, verify, and install the default model
  draft model verify [path]     Check a file against the pinned artifact
  draft model remove            Delete the installed model
  draft history list            Show recent transcripts
  draft history search <words>  Search transcripts
  draft history delete <id>     Delete one entry
  draft history clear           Delete every entry
  draft --version               Print the version
  draft --help                  Show this help

Transcribe options:
  --model <path>                GGUF model (required)
  --backend auto|vulkan|cpu     Compute backend (default: auto)
  --language <code>             ISO language hint
  --low-latency                 Skip temperature fallback (preview-style decode)
  --threads <n>                 CPU worker threads (default: library picks; env DRAFT_THREADS)
  --no-warmup                   Skip the untimed pipeline warm-up
  --timings                     Print decoder segment timings and pause stats
  --format                      Print the formatted text, not the raw transcript

Bench options:
  --model <path>                GGUF model (required)
  --fixtures <dir>              Directory of 16 kHz mono WAV files
  --runs <n>                    Measured runs per clip (default: 5)
  --warmup <n>                  Untimed runs before measuring
  --pause-ms <n>                Idle gap between measured runs
  --trace                       Print every measured run
  --json <path>                 Write the report as JSON
  --backend auto|vulkan|cpu     Compute backend (default: auto)

Mic options:
  --seconds <n>                 Recording length (default: 3)
  --out <path>                  Write a 16 kHz mono WAV
  --model <path>                Transcribe the recording after capture
  --backend auto|vulkan|cpu     Compute backend (default: auto)

Model options:
  --dir <path>                  Models folder (default: the Draft4 data folder)

History options:
  --limit <n>                   Entries to show (default: 20)
  --json                        Print entries as JSON

Listen options:
  --paste                       Insert the result at the current focus
  --hotkey <name>               right-ctrl (default), right-alt, f8, f9, pause"
    );
}

fn transcribe(args: &[String]) -> Result<()> {
    let options = parse_engine_args(args)?;
    let file = options
        .files
        .first()
        .context("transcribe needs one WAV file")?;
    if options.files.len() > 1 {
        bail!("transcribe accepts exactly one WAV file");
    }

    let mut engine = load_engine(&options)?;
    if !options.no_warmup {
        let started = Instant::now();
        engine.warm_up()?;
        eprintln!("warm-up: {:.0} ms", ms(started.elapsed().as_secs_f64()));
    }

    let clip = read_clip(file)?;
    let result = engine.transcribe(&clip.samples, &options.transcription)?;

    if options.format {
        let cleaned = text::clean_text(&result.text, false);
        let segments = result
            .segments
            .iter()
            .map(|segment| {
                format::Segment::new(segment.text.clone(), segment.start_ms, segment.end_ms)
            })
            .collect::<Vec<_>>();
        let format_options = format::Options {
            paragraphs: true,
            style: ParagraphStyle::Balanced,
            commands: true,
            lists: false,
            language: result.language.clone(),
        };
        // Read the stored profile so the CLI sees what the app would see. It is
        // never written here: a diagnostic run must not train the user's app.
        let prior = Rhythm::load_or_default(&Rhythm::default_path());
        let formatted = format::format(&cleaned, &segments, &format_options, &prior);
        eprintln!(
            "format: {} paragraph break(s), {} command(s), {} list item(s), prior median {} ms",
            formatted.breaks, formatted.commands, formatted.items, prior.median_gap_ms
        );
        println!("{}", formatted.text);
    } else {
        println!("{}", result.text);
    }
    eprintln!(
        "{}: {:.0} ms wall, {:.0} ms native (mel {:.0}, encode {:.0}, decode {:.0}), language {}",
        file.display(),
        ms(result.elapsed.as_secs_f64()),
        result.timings.total_ms(),
        result.timings.mel_ms,
        result.timings.encode_ms,
        result.timings.decode_ms,
        result.language.as_deref().unwrap_or("auto"),
    );
    if options.timings {
        print_timings(&result.segments);
    }
    Ok(())
}

/// Print the decoder rows and the pause statistics a formatting rule would see.
///
/// The text stays on stdout so `draftctl transcribe file.wav > out.txt` keeps
/// working; everything here goes to stderr. The median and p90 gap are printed
/// because they are the numbers the paragraph rule actually uses, and tuning
/// them against invented values is how a feature ships half done.
fn print_timings(segments: &[Segment]) {
    if segments.is_empty() {
        eprintln!("segments: none reported (pause-based formatting would do nothing)");
        return;
    }

    eprintln!("segments: {}", segments.len());
    let mut gaps = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        let gap = index
            .checked_sub(1)
            .map(|previous| segment.gap_before(&segments[previous]));
        if let Some(gap) = gap {
            gaps.push(gap);
        }
        let gap_text = gap.map_or_else(|| "     -".to_owned(), |gap| format!("{gap:>6}"));
        eprintln!(
            "  {index:>3} {:>7.2} -> {:>7.2} s  gap {gap_text} ms  {:?}",
            segment.start_ms as f64 / 1000.0,
            segment.end_ms as f64 / 1000.0,
            truncate(&segment.text, 72),
        );
    }

    if gaps.is_empty() {
        eprintln!("gaps: none (one segment, so there is no boundary to measure)");
        return;
    }
    gaps.sort_unstable();
    let median = gaps[gaps.len() / 2];
    let p90 = gaps[(gaps.len() * 9 / 10).min(gaps.len() - 1)];
    eprintln!(
        "gaps: {}  median {median} ms  p90 {p90} ms  max {} ms",
        gaps.len(),
        gaps[gaps.len() - 1]
    );
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut out = text.chars().take(limit).collect::<String>();
    out.push('…');
    out
}

fn mic(args: &[String]) -> Result<()> {
    let mut seconds = 3.0_f64;
    let mut out = None;
    let mut model = None;
    let mut backend = ComputeBackend::Auto;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--seconds" => {
                index += 1;
                seconds = args
                    .get(index)
                    .context("--seconds needs a number")?
                    .parse()
                    .context("--seconds needs a number")?;
            }
            "--out" => {
                index += 1;
                out = Some(PathBuf::from(
                    args.get(index).context("--out needs a path")?,
                ));
            }
            "--model" | "-m" => {
                index += 1;
                model = Some(PathBuf::from(
                    args.get(index).context("--model needs a path")?,
                ));
            }
            "--backend" => {
                index += 1;
                backend = parse_backend(args.get(index).context("--backend needs a value")?)?;
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => bail!("unknown mic option: {other}"),
        }
        index += 1;
    }

    if !(0.1..=600.0).contains(&seconds) {
        bail!("--seconds must be between 0.1 and 600");
    }

    eprintln!("recording {seconds:.1}s from the default microphone");
    let started = Instant::now();
    let recorder = draft_audio::Recorder::start()?;
    eprintln!(
        "device: {} at {} Hz",
        recorder.device(),
        recorder.sample_rate()
    );
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
    let recording = recorder.stop()?;
    eprintln!(
        "captured {:.2}s in {:.0} ms",
        recording.duration().as_secs_f64(),
        ms(started.elapsed().as_secs_f64())
    );

    if let Some(path) = &out {
        write_wav(path, &recording.samples)?;
        eprintln!("wrote {}", path.display());
    }

    if let Some(model) = model {
        let mut engine = Engine::load(&EngineConfig::new(model).backend(backend))
            .map_err(anyhow::Error::from)?;
        engine.warm_up()?;
        let result = engine.transcribe(&recording.samples, &TranscriptionOptions::default())?;
        println!("{}", result.text);
        eprintln!(
            "{:.0} ms wall, {:.0} ms native",
            ms(result.elapsed.as_secs_f64()),
            result.timings.total_ms()
        );
    }

    Ok(())
}

fn model(args: &[String]) -> Result<()> {
    let mut dir = draft_core::paths::models_dir();
    let mut rest: Vec<&String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dir" => {
                dir = args
                    .get(index + 1)
                    .map(PathBuf::from)
                    .context("--dir needs a path")?;
                index += 2;
            }
            _ => {
                rest.push(&args[index]);
                index += 1;
            }
        }
    }

    match rest.first().map(|argument| argument.as_str()) {
        None | Some("status") => model_status(&dir),
        Some("list") => model_list(&dir),
        Some("install") => model_install(&dir),
        Some("verify") => model_verify(rest.get(1).map(PathBuf::from), &dir),
        Some("remove") => model_remove(&dir),
        Some("--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some(other) => bail!("unknown model command: {other} (try `draft model --help`)"),
    }
}

fn model_status(dir: &Path) -> Result<()> {
    let spec = &draft_core::models::DEFAULT_MODEL;
    let state = draft_core::models::state(spec, dir)?;

    println!("Model   {} ({})", spec.name, spec.quantization);
    println!("Id      {}", spec.id);
    println!("Folder  {}", dir.display());
    match state {
        draft_core::models::ModelState::Missing => {
            println!("State   missing - run `draft model install`");
        }
        draft_core::models::ModelState::Partial { bytes } => {
            println!(
                "State   partial, {} of {} - run `draft model install` to resume",
                human_bytes(bytes),
                human_bytes(spec.size)
            );
        }
        draft_core::models::ModelState::Installed { bytes } => {
            println!("State   installed ({})", human_bytes(bytes));
            println!("File    {}", spec.path_in(dir).display());
        }
        draft_core::models::ModelState::Corrupt { bytes } => {
            println!(
                "State   wrong length ({bytes} bytes, expected {}) - run `draft model install`",
                spec.size
            );
        }
    }
    Ok(())
}

fn model_list(dir: &Path) -> Result<()> {
    for spec in draft_core::models::CATALOG {
        let state = draft_core::models::state(spec, dir)?;
        let status = match state {
            draft_core::models::ModelState::Missing => "missing".to_owned(),
            draft_core::models::ModelState::Partial { bytes } => {
                format!("partial ({})", human_bytes(bytes))
            }
            draft_core::models::ModelState::Installed { bytes } => {
                format!("installed ({})", human_bytes(bytes))
            }
            draft_core::models::ModelState::Corrupt { bytes } => {
                format!("wrong length ({bytes} bytes)")
            }
        };
        println!(
            "{:<32} {:<6} {:>10}  {status}",
            spec.id,
            spec.quantization,
            human_bytes(spec.size)
        );
    }
    Ok(())
}

fn model_install(dir: &Path) -> Result<()> {
    use std::io::Write as _;
    use std::sync::atomic::AtomicBool;

    let spec = &draft_core::models::DEFAULT_MODEL;
    eprintln!(
        "Installing {} ({}), {} to {}",
        spec.name,
        spec.quantization,
        human_bytes(spec.size),
        spec.path_in(dir).display()
    );

    let source = draft_core::models::HttpSource::new(spec);
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let mut last_report = started;
    let mut reported = false;

    let path = draft_core::models::install(spec, dir, &source, &cancel, |progress| {
        let now = Instant::now();
        if now.duration_since(last_report) >= Duration::from_millis(250)
            || progress.downloaded == progress.total
        {
            let percent = if progress.total == 0 {
                0.0
            } else {
                progress.downloaded as f64 / progress.total as f64 * 100.0
            };
            eprint!(
                "\r  {percent:>5.1}%  {} / {}",
                human_bytes(progress.downloaded),
                human_bytes(progress.total)
            );
            let _ = std::io::stderr().flush();
            last_report = now;
            reported = true;
        }
    })?;

    if reported {
        eprintln!();
    }
    println!(
        "Installed and verified {} in {:.1}s",
        path.display(),
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn model_verify(path: Option<PathBuf>, dir: &Path) -> Result<()> {
    let spec = &draft_core::models::DEFAULT_MODEL;
    let path = path.unwrap_or_else(|| spec.path_in(dir));
    println!("File     {}", path.display());
    println!("Expected {} bytes", spec.size);
    println!("         sha256 {}", spec.sha256);

    let report = draft_core::models::verify(&path, spec)?;
    println!("Actual   {} bytes", report.bytes);
    println!("         sha256 {}", report.sha256);
    if report.matches(spec) {
        println!("OK - exact pinned artifact");
        Ok(())
    } else {
        bail!("{} does not match the pinned artifact", path.display());
    }
}

fn model_remove(dir: &Path) -> Result<()> {
    let spec = &draft_core::models::DEFAULT_MODEL;
    if draft_core::models::remove(spec, dir)? {
        println!("Removed {}", spec.path_in(dir).display());
    } else {
        println!("Nothing to remove in {}", dir.display());
    }
    Ok(())
}

fn human_bytes(bytes: u64) -> String {
    const GB: f64 = 1_000_000_000.0;
    const MB: f64 = 1_000_000.0;
    if bytes as f64 >= GB {
        format!("{:.2} GB", bytes as f64 / GB)
    } else {
        format!("{:.1} MB", bytes as f64 / MB)
    }
}

fn history(args: &[String]) -> Result<()> {
    let mut limit = 20usize;
    let mut json = false;
    let mut rest: Vec<&String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--limit" => {
                limit = args
                    .get(index + 1)
                    .context("--limit needs a number")?
                    .parse()
                    .context("--limit needs a number")?;
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            _ => {
                rest.push(&args[index]);
                index += 1;
            }
        }
    }

    match rest.first().map(|argument| argument.as_str()) {
        None | Some("list") => {
            let history = open_history()?;
            print_history(&history.recent(limit), json)
        }
        Some("search") => {
            let query = rest[1..]
                .iter()
                .map(|argument| argument.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            if query.trim().is_empty() {
                bail!("history search needs words to look for");
            }
            let history = open_history()?;
            print_history(&history.search(&query, limit), json)
        }
        Some("delete") => {
            let id = rest.get(1).context("history delete needs an entry id")?;
            let mut history = open_history()?;
            if history.delete(id)? {
                println!("Deleted {id}");
                Ok(())
            } else {
                bail!("no history entry with id {id}")
            }
        }
        Some("clear") => {
            let mut history = open_history()?;
            let count = history.entries().len();
            history.clear()?;
            println!("Cleared {count} entries from {}", history.path().display());
            Ok(())
        }
        Some("--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some(other) => bail!("unknown history command: {other} (try `draft history --help`)"),
    }
}

fn open_history() -> Result<draft_core::history::History> {
    let path = draft_core::paths::history_path();
    let history = draft_core::history::History::open(
        path,
        draft_core::history::HistoryPolicy::default(),
        draft_core::history::now_ms(),
    )?;
    if history.recovered_from_corruption {
        eprintln!("warning: the previous history file was malformed and was moved aside");
    }
    Ok(history)
}

fn print_history(entries: &[&draft_core::history::HistoryEntry], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(entries)?);
        return Ok(());
    }
    if entries.is_empty() {
        println!("No history entries.");
        return Ok(());
    }
    for entry in entries {
        let duration = entry
            .duration_ms
            .map(|ms| format!("{:>5.1}s", ms as f64 / 1000.0))
            .unwrap_or_else(|| "     -".to_owned());
        let text = entry.text.replace(['\n', '\r'], " ");
        println!(
            "{}  {}  {:18}  {}",
            format_utc(entry.created_at_ms),
            duration,
            entry.id,
            text
        );
    }
    Ok(())
}

/// `YYYY-MM-DD HH:MM` in UTC. History timestamps are stored as Unix millis; a
/// portable display without a timezone database stays honest by saying UTC.
fn format_utc(ms: i64) -> String {
    let seconds = ms.div_euclid(1_000);
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60
    )
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day as u32)
}

struct Job {
    samples: Vec<f32>,
    index: usize,
    captured: Duration,
}

fn listen(args: &[String]) -> Result<()> {
    let options = parse_engine_args(args)?;
    if !options.files.is_empty() {
        bail!("listen takes no positional files");
    }

    let mut engine = load_engine(&options)?;
    engine.warm_up()?;
    let paste = options.paste;
    let hotkey = options.hotkey;
    let transcription = options.transcription;

    // The engine runs on its own worker so a new hold can start while the
    // previous clip is still decoding. Jobs stay in submission order.
    let (jobs, job_receiver) = std::sync::mpsc::channel::<Job>();
    let worker = std::thread::Builder::new()
        .name("draft-transcriber".to_owned())
        .spawn(move || {
            while let Ok(job) = job_receiver.recv() {
                let started = Instant::now();
                match engine.transcribe(&job.samples, &transcription) {
                    Ok(result) => {
                        let cleaned = text::clean_text(&result.text, false);
                        if cleaned.is_empty() || !text::is_meaningful(&cleaned) {
                            eprintln!(
                                "[{}] empty in {:.0} ms",
                                job.index,
                                ms(started.elapsed().as_secs_f64())
                            );
                        } else {
                            println!("{cleaned}");
                            if paste {
                                if let Err(error) = draft_paste::paste_text(&cleaned) {
                                    eprintln!("[{}] paste failed: {error:#}", job.index);
                                }
                            }
                            eprintln!(
                                "[{}] {:.0} ms engine, {:.0} ms from press, {:.1}s audio",
                                job.index,
                                ms(started.elapsed().as_secs_f64()),
                                ms((job.captured + started.elapsed()).as_secs_f64()),
                                job.samples.len() as f64 / f64::from(SAMPLE_RATE),
                            );
                        }
                    }
                    Err(error) => {
                        eprintln!("[{}] transcription failed: {error}", job.index);
                    }
                }
            }
        })
        .context("spawn transcriber thread")?;

    let (_listener, events) = draft_hotkey::start(hotkey)?;
    println!(
        "Ready. Hold {} to dictate. Escape cancels. Ctrl+C quits.",
        hotkey.name()
    );

    let mut recorder: Option<draft_audio::Recorder> = None;
    let mut pressed_at = Instant::now();
    let mut index = 0_usize;

    for event in events {
        match event {
            HotkeyEvent::Pressed => {
                if recorder.is_some() {
                    continue;
                }
                pressed_at = Instant::now();
                match draft_audio::Recorder::start() {
                    Ok(started) => recorder = Some(started),
                    Err(error) => eprintln!("microphone: {error:#}"),
                }
            }
            HotkeyEvent::Released => {
                let Some(recorder) = recorder.take() else {
                    continue;
                };
                match recorder.stop() {
                    Ok(recording) => {
                        if recording.duration() < Duration::from_millis(300) {
                            eprintln!(
                                "discarded {:.0} ms capture",
                                ms(recording.duration().as_secs_f64())
                            );
                            continue;
                        }
                        index += 1;
                        let job = Job {
                            samples: recording.samples,
                            index,
                            captured: pressed_at.elapsed(),
                        };
                        if jobs.send(job).is_err() {
                            eprintln!("transcriber stopped");
                            break;
                        }
                    }
                    Err(error) => eprintln!("microphone: {error:#}"),
                }
            }
            HotkeyEvent::Cancel => {
                if let Some(recorder) = recorder.take() {
                    let _ = recorder.stop();
                    eprintln!("cancelled");
                }
            }
        }
    }

    drop(jobs);
    let _ = worker.join();
    Ok(())
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("create {}", path.display()))?;
    for &sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        writer
            .write_sample(value)
            .with_context(|| format!("write {}", path.display()))?;
    }
    writer
        .finalize()
        .with_context(|| format!("finalize {}", path.display()))?;
    Ok(())
}

fn bench(args: &[String]) -> Result<()> {
    let options = parse_engine_args(args)?;
    let fixtures = if let Some(dir) = &options.fixtures {
        collect_wavs(dir)?
    } else {
        options.files.clone()
    };
    if fixtures.is_empty() {
        bail!("bench needs --fixtures <dir> or at least one WAV file");
    }

    let mut engine = load_engine(&options)?;
    eprintln!(
        "engine: backend {}, device {}, load {:.0} ms",
        engine.backend(),
        engine.device(),
        ms(engine.load_time().as_secs_f64())
    );

    // Warm the backend on the first clip so the first measured run is not
    // paying for pipeline compilation.
    let first = read_clip(&fixtures[0])?;
    for index in 0..options.warmup {
        let started = Instant::now();
        let _ = engine.transcribe(&first.samples, &options.transcription)?;
        eprintln!(
            "warm-up {}: {:.0} ms",
            index + 1,
            ms(started.elapsed().as_secs_f64())
        );
    }

    let mut report = BTreeMap::new();
    let mut table = Vec::new();

    for path in &fixtures {
        let clip = read_clip(path)?;
        let mut totals = Vec::with_capacity(options.runs);
        let mut timings = Vec::with_capacity(options.runs);
        let mut last_text = String::new();

        for run in 0..options.runs {
            let started = Instant::now();
            let result = engine.transcribe(&clip.samples, &options.transcription)?;
            let elapsed = ms(started.elapsed().as_secs_f64());
            if options.trace {
                let preview: String = result.text.chars().take(60).collect();
                eprintln!(
                    "  run {:>2}: {elapsed:>7.0} ms, {:>4} chars {preview:?}",
                    run + 1,
                    result.text.chars().count()
                );
            }
            totals.push(elapsed);
            timings.push(result.timings);
            last_text = result.text;
            if options.pause_ms > 0 && run + 1 < options.runs {
                std::thread::sleep(Duration::from_millis(options.pause_ms as u64));
            }
        }

        totals.sort_by(f64::total_cmp);
        let p50 = percentile(&totals, 0.50);
        let p95 = percentile(&totals, 0.95);
        let rtf = clip.duration_s / (p50 / 1000.0);
        let stages = average_stages(&timings);

        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        table.push(format!(
            "{name:<16} {duration:>6.1}s  p50={p50:>7.0}ms  p95={p95:>7.0}ms  {rtf:>5.1}x",
            duration = clip.duration_s,
        ));
        report.insert(
            name,
            FixtureReport {
                duration_s: clip.duration_s,
                final_ms: Percentiles { p50, p95 },
                rtf,
                stages_ms: Stages {
                    mel: stages.mel_ms as f64,
                    encode: stages.encode_ms as f64,
                    decode: stages.decode_ms as f64,
                },
                text: last_text,
            },
        );
    }

    for line in &table {
        println!("{line}");
    }

    let payload = BenchReport {
        created_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default(),
        machine: Machine {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
        },
        engine: EngineReport {
            name: "Draft (Rust)".to_owned(),
            model: options.model.display().to_string(),
            backend: engine.backend().to_owned(),
            device: engine.device().to_owned(),
            warmup: options.warmup,
            iters: options.runs,
        },
        fixtures: report,
    };

    if let Some(path) = &options.json {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(&payload)?;
        std::fs::write(path, json).with_context(|| format!("write {}", path.display()))?;
        eprintln!("wrote {}", path.display());
    }

    Ok(())
}

#[derive(Debug)]
struct EngineArgs {
    model: PathBuf,
    backend: ComputeBackend,
    fixtures: Option<PathBuf>,
    json: Option<PathBuf>,
    runs: usize,
    warmup: usize,
    pause_ms: usize,
    no_warmup: bool,
    paste: bool,
    trace: bool,
    /// CPU worker threads; `None` uses the library default.
    threads: Option<i32>,
    /// Print the decoder's segment timings after the text. Used to inspect real
    /// clips before trusting any pause-based rule.
    timings: bool,
    /// Print the text after the formatting pass instead of the raw transcript.
    /// Uses the same rules and the same stored pause profile as the app.
    format: bool,
    hotkey: HotkeyConfig,
    transcription: TranscriptionOptions,
    files: Vec<PathBuf>,
}

fn parse_engine_args(args: &[String]) -> Result<EngineArgs> {
    let mut model = None;
    let mut backend = ComputeBackend::Auto;
    let mut fixtures = None;
    let mut json = None;
    let mut runs = 5;
    let mut warmup = 2;
    let mut pause_ms = 0;
    let mut no_warmup = false;
    let mut paste = false;
    let mut trace = false;
    let mut threads = None;
    let mut timings = false;
    let mut format = false;
    let mut hotkey = HotkeyConfig::default();
    let mut language = None;
    let mut vocabulary = Vec::new();
    let mut prompt = None;
    let mut low_latency = false;
    let mut files = Vec::new();

    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--model" | "-m" => {
                index += 1;
                model = Some(PathBuf::from(
                    args.get(index)
                        .with_context(|| format!("{arg} needs a path"))?,
                ));
            }
            "--backend" => {
                index += 1;
                backend = parse_backend(
                    args.get(index)
                        .with_context(|| format!("{arg} needs a value"))?,
                )?;
            }
            "--fixtures" => {
                index += 1;
                fixtures = Some(PathBuf::from(
                    args.get(index)
                        .with_context(|| format!("{arg} needs a directory"))?,
                ));
            }
            "--json" => {
                index += 1;
                json = Some(PathBuf::from(
                    args.get(index)
                        .with_context(|| format!("{arg} needs a path"))?,
                ));
            }
            "--runs" => {
                index += 1;
                runs = parse_number(args.get(index), arg)?;
            }
            "--warmup" => {
                index += 1;
                warmup = parse_number(args.get(index), arg)?;
            }
            "--pause-ms" => {
                index += 1;
                pause_ms = parse_number(args.get(index), arg)?;
            }
            "--language" => {
                index += 1;
                language = Some(
                    args.get(index)
                        .with_context(|| format!("{arg} needs a code"))?
                        .clone(),
                );
            }
            "--vocabulary" => {
                index += 1;
                let value = args
                    .get(index)
                    .with_context(|| format!("{arg} needs a comma-separated list"))?;
                vocabulary.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|term| !term.is_empty())
                        .map(str::to_owned),
                );
            }
            "--prompt" => {
                index += 1;
                prompt = Some(
                    args.get(index)
                        .with_context(|| format!("{arg} needs text"))?
                        .clone(),
                );
            }
            "--no-warmup" => no_warmup = true,
            "--threads" => {
                index += 1;
                threads = Some(
                    args.get(index)
                        .with_context(|| format!("{arg} needs a number"))?
                        .parse()
                        .with_context(|| format!("{arg} needs a number"))?,
                );
            }
            "--low-latency" => low_latency = true,
            "--paste" => paste = true,
            "--trace" => trace = true,
            "--timings" => timings = true,
            "--format" => format = true,
            "--hotkey" => {
                index += 1;
                let value = args.get(index).with_context(|| {
                    format!("{arg} needs a value (right-ctrl, right-alt, f8, f9, pause)")
                })?;
                hotkey = HotkeyConfig::named(value)
                    .with_context(|| format!("unknown hotkey: {value}"))?;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other if other.starts_with('-') => bail!("unknown option: {other}"),
            other => files.push(PathBuf::from(other)),
        }
        index += 1;
    }

    let model = model.context("--model is required")?;
    if runs == 0 {
        bail!("--runs must be at least 1");
    }

    Ok(EngineArgs {
        model,
        backend,
        fixtures,
        json,
        runs,
        warmup,
        pause_ms,
        no_warmup,
        paste,
        trace,
        threads,
        timings,
        format,
        hotkey,
        transcription: TranscriptionOptions {
            language,
            vocabulary,
            prompt,
            low_latency,
        },
        files,
    })
}

fn parse_backend(value: &str) -> Result<ComputeBackend> {
    match value {
        "auto" => Ok(ComputeBackend::Auto),
        "vulkan" => Ok(ComputeBackend::Vulkan),
        "metal" => Ok(ComputeBackend::Metal),
        "cuda" => Ok(ComputeBackend::Cuda),
        "cpu" => Ok(ComputeBackend::Cpu),
        other => bail!("unknown backend: {other}"),
    }
}

fn parse_number(value: Option<&String>, flag: &str) -> Result<usize> {
    value
        .with_context(|| format!("{flag} needs a number"))?
        .parse()
        .with_context(|| format!("{flag} needs a number"))
}

fn load_engine(options: &EngineArgs) -> Result<Engine> {
    Engine::load(
        &EngineConfig::new(&options.model)
            .backend(options.backend)
            .threads(options.threads),
    )
    .map_err(anyhow::Error::from)
}

fn collect_wavs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

struct Clip {
    samples: Vec<f32>,
    duration_s: f64,
}

fn read_clip(path: &Path) -> Result<Clip> {
    let mut reader =
        hound::WavReader::open(path).with_context(|| format!("open {}", path.display()))?;
    let spec = reader.spec();
    if spec.channels != 1 {
        bail!(
            "{}: expected mono, found {} channels",
            path.display(),
            spec.channels
        );
    }
    if spec.sample_rate != SAMPLE_RATE {
        bail!(
            "{}: expected {} Hz, found {} Hz",
            path.display(),
            SAMPLE_RATE,
            spec.sample_rate
        );
    }

    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("decode {}", path.display()))?,
        hound::SampleFormat::Int => match spec.bits_per_sample {
            16 => reader
                .samples::<i16>()
                .map(|sample| sample.map(|value| f32::from(value) / f32::from(i16::MAX)))
                .collect::<std::result::Result<Vec<_>, _>>()
                .with_context(|| format!("decode {}", path.display()))?,
            32 => reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / 2_147_483_648.0))
                .collect::<std::result::Result<Vec<_>, _>>()
                .with_context(|| format!("decode {}", path.display()))?,
            bits => bail!("{}: unsupported {bits}-bit PCM", path.display()),
        },
    };

    let duration_s = samples.len() as f64 / f64::from(SAMPLE_RATE);
    Ok(Clip {
        samples,
        duration_s,
    })
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[index]
}

fn average_stages(timings: &[EngineTimings]) -> EngineTimings {
    if timings.is_empty() {
        return EngineTimings::default();
    }
    let count = timings.len() as f32;
    EngineTimings {
        mel_ms: timings.iter().map(|timing| timing.mel_ms).sum::<f32>() / count,
        encode_ms: timings.iter().map(|timing| timing.encode_ms).sum::<f32>() / count,
        decode_ms: timings.iter().map(|timing| timing.decode_ms).sum::<f32>() / count,
    }
}

fn ms(seconds: f64) -> f64 {
    seconds * 1000.0
}

#[derive(Serialize)]
struct BenchReport {
    created_unix: u64,
    machine: Machine,
    engine: EngineReport,
    fixtures: BTreeMap<String, FixtureReport>,
}

#[derive(Serialize)]
struct Machine {
    os: String,
    arch: String,
}

#[derive(Serialize)]
struct EngineReport {
    name: String,
    model: String,
    backend: String,
    device: String,
    warmup: usize,
    iters: usize,
}

#[derive(Serialize)]
struct FixtureReport {
    duration_s: f64,
    final_ms: Percentiles,
    rtf: f64,
    stages_ms: Stages,
    text: String,
}

#[derive(Serialize)]
struct Percentiles {
    p50: f64,
    p95: f64,
}

#[derive(Serialize)]
struct Stages {
    mel: f64,
    encode: f64,
    decode: f64,
}
