//! Microphone capture for Draft.
//!
//! The recorder returns 16 kHz mono `f32` PCM, ready for the engine. It keeps
//! the device's native rate and channel layout while capturing, then downmixes
//! and resamples once recording has stopped, so the real-time callback stays
//! small.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream, StreamConfig};
use rubato::{FftFixedIn, Resampler};
use tracing::{info, warn};

pub const OUTPUT_RATE: u32 = 16_000;

/// A finished recording at 16 kHz mono.
#[derive(Debug, Clone, Default)]
pub struct Recording {
    pub samples: Vec<f32>,
}

impl Recording {
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.samples.len() as f64 / f64::from(OUTPUT_RATE))
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// A capture in progress. Keep it on the thread that created it.
pub struct Recorder {
    stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
    sample_rate: u32,
    channels: usize,
    device: String,
    fell_back: bool,
}

impl Recorder {
    /// Open the default microphone and start capturing.
    pub fn start() -> Result<Self> {
        Self::start_on(None)
    }

    /// Open a named microphone, falling back to the system default when the
    /// named device is no longer available. A fallback is reported instead of
    /// silently rewriting the user's saved choice.
    pub fn start_on(device_name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();
        let mut fell_back = false;
        let device = match device_name {
            Some(name) => {
                let selected = host.input_devices().ok().and_then(|mut devices| {
                    devices.find(|device| device.name().ok().as_deref() == Some(name))
                });
                match selected {
                    Some(device) => device,
                    None => {
                        warn!(
                            requested = name,
                            "microphone is unavailable; using the default"
                        );
                        fell_back = true;
                        host.default_input_device()
                            .context("no default input device after the selected one disappeared")?
                    }
                }
            }
            None => host
                .default_input_device()
                .context("no default input device")?,
        };
        let name = device.name().unwrap_or_else(|_| "default".to_owned());
        let supported = device
            .default_input_config()
            .with_context(|| format!("read input config for {name}"))?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate.0;
        let channels = usize::from(config.channels);
        let samples = Arc::new(Mutex::new(Vec::new()));
        let level = Arc::new(AtomicU32::new(0.0_f32.to_bits()));

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, &config, &samples, &level, channels),
            SampleFormat::I16 => build_stream::<i16>(&device, &config, &samples, &level, channels),
            SampleFormat::U16 => build_stream::<u16>(&device, &config, &samples, &level, channels),
            other => bail!("unsupported sample format: {other:?}"),
        }?;
        stream.play().context("start microphone stream")?;

        info!(
            device = %name,
            sample_rate,
            channels,
            format = ?sample_format,
            fell_back,
            "microphone open"
        );

        Ok(Self {
            stream,
            samples,
            level,
            sample_rate,
            channels,
            device: name,
            fell_back,
        })
    }

    pub fn device(&self) -> &str {
        &self.device
    }

    /// The most recent peak amplitude, smoothed by the capture callback.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    /// True when the requested device vanished and the default was opened.
    pub fn fell_back(&self) -> bool {
        self.fell_back
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Stop the stream and return the clip as 16 kHz mono.
    pub fn stop(self) -> Result<Recording> {
        drop(self.stream);
        let interleaved = std::mem::take(&mut *self.samples.lock().expect("audio buffer poisoned"));
        let mono = downmix(&interleaved, self.channels);
        let samples = resample(&mono, self.sample_rate, OUTPUT_RATE)?;
        Ok(Recording { samples })
    }

    /// A 16 kHz mono snapshot of the most recent `seconds` of capture.
    ///
    /// Live previews only need the tail, so this clones a bounded slice of the
    /// raw buffer instead of the whole recording. The capture callback keeps
    /// writing while the copy is resampled off the audio thread.
    pub fn preview(&self, seconds: u32) -> Result<Recording> {
        let sample_limit = seconds as usize * self.sample_rate as usize * self.channels;
        let interleaved = {
            let buffer = self.samples.lock().expect("audio buffer poisoned");
            let start = buffer.len().saturating_sub(sample_limit);
            buffer[start..].to_vec()
        };
        let mono = downmix(&interleaved, self.channels);
        let samples = resample(&mono, self.sample_rate, OUTPUT_RATE)?;
        Ok(Recording { samples })
    }
}

/// Record for a fixed duration. Useful for diagnostics and smoke tests.
pub fn record_for(duration: Duration) -> Result<Recording> {
    let recorder = Recorder::start()?;
    std::thread::sleep(duration);
    recorder.stop()
}

/// Names of every input device the host exposes.
pub fn input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices = host.input_devices().context("enumerate input devices")?;
    let mut names = Vec::new();
    for device in devices {
        if let Ok(name) = device.name() {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    Ok(names)
}

/// The device cpal considers the system default, if one is available.
pub fn default_input_device_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|device| device.name().ok())
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    samples: &Arc<Mutex<Vec<f32>>>,
    level: &Arc<AtomicU32>,
    channels: usize,
) -> Result<Stream>
where
    T: Sample + cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let samples = Arc::clone(samples);
    let level = Arc::clone(level);
    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let mut buffer = samples.lock().expect("audio buffer poisoned");
                buffer.reserve(data.len());
                let mut peak = 0.0_f32;
                for &sample in data {
                    let sample = sample.to_sample::<f32>();
                    peak = peak.max(sample.abs());
                    buffer.push(sample);
                }
                // Decay so a level meter settles on silence instead of
                // freezing at the last loud buffer.
                let previous = f32::from_bits(level.load(Ordering::Relaxed));
                let next = peak.max(previous * 0.82);
                level.store(next.to_bits(), Ordering::Relaxed);
                // Bound a stuck recording at roughly ten minutes.
                let cap = 10 * 60 * 48_000 * channels;
                if buffer.len() > cap {
                    let excess = buffer.len() - cap;
                    buffer.drain(..excess);
                }
            },
            |error| tracing::error!(%error, "microphone stream error"),
            None,
        )
        .context("build microphone stream")?;
    Ok(stream)
}

fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn resample(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    if from == to || input.is_empty() {
        return Ok(input.to_vec());
    }

    const CHUNK: usize = 1024;
    let mut resampler = FftFixedIn::<f32>::new(from as usize, to as usize, CHUNK, 2, 1)
        .context("create resampler")?;
    let delay = resampler.output_delay();
    let expected = (input.len() as f64 * f64::from(to) / f64::from(from)).round() as usize;
    let mut output = Vec::with_capacity(expected + CHUNK + delay);

    let padded_len = input.len().div_ceil(CHUNK) * CHUNK;
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(input);
    padded.resize(padded_len, 0.0);

    for input_chunk in padded.chunks(CHUNK) {
        let waves = resampler
            .process(&[input_chunk.to_vec()], None)
            .context("resample microphone audio")?;
        output.extend_from_slice(&waves[0]);
    }

    // The FFT resampler emits whole internal blocks and delays its output.
    // Feed silence until the delayed real audio and the requested length are
    // both available. Without this the last few milliseconds of every clip
    // were simply dropped.
    let mut guard = 0;
    while output.len() < expected + delay && guard < 64 {
        let waves = resampler
            .process(&[vec![0.0; CHUNK]], None)
            .context("flush resampler")?;
        output.extend_from_slice(&waves[0]);
        guard += 1;
    }

    if output.len() > delay {
        output.drain(..delay);
    } else {
        output.clear();
    }
    output.resize(expected, 0.0);
    output.truncate(expected);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{downmix, resample};

    #[test]
    fn downmix_averages_channels() {
        assert_eq!(downmix(&[1.0, -1.0, 0.5, 0.5], 2), vec![0.0, 0.5]);
    }

    #[test]
    fn resample_preserves_duration() {
        let input = vec![0.0_f32; 48_000];
        let output = resample(&input, 48_000, 16_000).unwrap();
        assert_eq!(output.len(), 16_000);
    }

    #[test]
    fn resample_preserves_the_signal() {
        let from = 48_000;
        let to = 16_000;
        let hz = 440.0_f32;
        let input: Vec<f32> = (0..from)
            .map(|index| (std::f32::consts::TAU * hz * index as f32 / from as f32).sin())
            .collect();

        let output = resample(&input, from, to).unwrap();
        assert_eq!(output.len(), to as usize);

        // Ignore the resampler's short edge transients. The middle 0.9 s must
        // still contain a 440 Hz tone at full amplitude; duration alone would
        // pass even if the samples were corrupted.
        let middle = &output[800..15_200];
        let crossings = middle
            .windows(2)
            .filter(|pair| (pair[0] <= 0.0) != (pair[1] <= 0.0))
            .count();
        let expected = (hz * middle.len() as f32 / to as f32 * 2.0).round() as i32;
        assert!(
            (crossings as i32 - expected).abs() <= 4,
            "expected {expected} zero crossings, got {crossings}"
        );

        let peak = middle
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        assert!((0.95..=1.05).contains(&peak), "peak amplitude was {peak}");
    }
}
