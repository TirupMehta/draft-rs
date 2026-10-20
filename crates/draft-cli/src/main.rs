use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use draft_core::text;
use draft_engine::{ComputeBackend, Engine, EngineConfig, EngineTimings, TranscriptionOptions};
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
  draft --version               Print the version
  draft --help                  Show this help

Transcribe options:
  --model <path>                GGUF model (required)
  --backend auto|vulkan|cpu     Compute backend (default: auto)
  --language <code>             ISO language hint
  --no-warmup                   Skip the untimed pipeline warm-up

Bench options:
  --model <path>                GGUF model (required)
  --fixtures <dir>              Directory of 16 kHz mono WAV files
  --runs <n>                    Measured runs per clip (default: 5)
  --warmup <n>                  Untimed runs before measuring
  --trace                       Print every measured runng (default: 2)
  --json <path>                 Write the report as JSON
  --backend auto|vulkan|cpu     Compute backend (default: auto)

Mic options:
  --seconds <n>                 Recording length (default: 3)
  --out <path>                  Write a 16 kHz mono WAV
  --model <path>                Transcribe the recording after capture
  --backend auto|vulkan|cpu     Compute backend (default: auto)

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

    println!("{}", result.text);
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
    Ok(())
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
        hotkey.name
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
    no_warmup: bool,
    paste: bool,
    trace: bool,
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
    let mut no_warmup = false;
    let mut paste = false;
    let mut trace = false;
    let mut hotkey = HotkeyConfig::default();
    let mut language = None;
    let mut vocabulary = Vec::new();
    let mut prompt = None;
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
            "--paste" => paste = true,
            "--trace" => trace = true,
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
        no_warmup,
        paste,
        trace,
        hotkey,
        transcription: TranscriptionOptions {
            language,
            vocabulary,
            prompt,
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
    Engine::load(&EngineConfig::new(&options.model).backend(options.backend))
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
