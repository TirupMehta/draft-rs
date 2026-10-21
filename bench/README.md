# Benchmarks

Performance is a release gate. The reference machine, the Draft v3.5 baseline,
and the pass/fail rule live in [`docs/PERFORMANCE.md`](../docs/PERFORMANCE.md).

## Fixtures

Audio is not committed: it is large, personal, and easy to regenerate. Put
16 kHz mono WAVs in `bench/fixtures/` (gitignored):

| File | Content |
| --- | --- |
| `tts-3s.wav` | Greeting, one sentence |
| `tts-5s.wav` | Two sentences, name |
| `tts-15s.wav` | Names, date, list, punctuation |
| `tts-30s.wav` | Two paragraphs |
| `tts-60s.wav` | Four paragraphs |
| `real-jfk.wav` | Public-domain real speech for an accuracy sanity check |
| `multilingual-10s.wav` | Optional: non-English sentence with a proper noun |

Record all fixtures with the same microphone and room for a fair comparison.
Do not commit personal audio.

For a quick, reproducible latency set, prepare 16 kHz TTS clips and trim them to
exact durations:

```text
python .\bench\trim_fixtures.py
```

TTS clips are fine for latency and engine bake-offs. Use real recordings for
WER and multilingual quality.

For a quick real-speech fixture, download Whisper's public-domain JFK sample:

```text
curl -L https://raw.githubusercontent.com/ggerganov/whisper.cpp/master/samples/jfk.wav -o bench\fixtures\real-jfk.wav
```

## Running

`cargo run -p draft` runs the `draftctl` development binary:

```text
cargo run -p draft -- bench --model "%LOCALAPPDATA%\Draft4\models\whisper-large-v3-turbo-F16.gguf" --backend vulkan --fixtures bench/fixtures --warmup 2 --runs 10 --json bench/results/rust.json
```

The harness loads one model, runs the warm-up passes, then measures each clip.
It reports p50/p95 and the engine stage timings; `compare_bakeoff.py` compares
the result with `bench/baseline.json`.

## Engine bake-off

The Windows engine is `transcribe.cpp` with Vulkan. Build `transcribe-cli` and
`transcribe-bench` from a short path (for example `C:\bv`), with
`-DTRANSCRIBE_VULKAN=ON -DTRANSCRIBE_BUILD_TOOLS=ON`, then:

```text
C:\bv\bin\Release\transcribe-bench.exe --model "%LOCALAPPDATA%\Draft4\models\whisper-large-v3-turbo-F16.gguf" --sample .\bench\fixtures\tts-5s.wav --backend vulkan --device 0 --warmup 3 --iters 5 --quiet --json-out .\bench\results\vulkan-tts-5s.wav.json

python .\bench\compare_bakeoff.py --results-dir .\bench\results --prefix vulkan-
```

Always warm up: the first Vulkan run compiles pipelines and is not a real
latency number. Results are in [`docs/PERFORMANCE.md`](../docs/PERFORMANCE.md);
the measured Vulkan path is 2.5-4.2x faster than v3.5 across the 3-60 second
clips.

## Baseline schema

`bench/baseline.json` is committed only after a real measurement on the
reference machine. No placeholder numbers.

```json
{
  "created": "2026-10-20T08:04:40Z",
  "machine": {
    "cpu": "Intel Core i7-13620H",
    "gpu": "NVIDIA GeForce RTX 4060 Laptop 8 GB",
    "ram_gb": 16,
    "os": "Windows 11 build 26300"
  },
  "engine": {
    "name": "Draft v3.5",
    "model": "large-v3-turbo",
    "backend": "CTranslate2 CUDA",
    "compute_type": "float16",
    "beam_size": 5,
    "vad": true
  },
  "fixtures": {
    "tts-5s.wav": {
      "duration_s": 5.0,
      "final_ms": { "p50": 880.1, "p95": 929.4 },
      "rtf": 5.68,
      "text": "Hold the shortcut, speak naturally, and release. The final"
    }
  }
}
```

The Rust harness adds `paste_ms` and `first_partial_ms`; the v3.5 engine only
measured final decode. See [`baseline.json`](baseline.json) for the full
recorded measurement.
