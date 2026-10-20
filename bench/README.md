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
| `multilingual-10s.wav` | Optional: non-English sentence with a proper noun |

Record all fixtures with the same microphone and room for a fair comparison.
Do not commit personal audio.

For a quick, reproducible latency set, generate local Windows TTS clips and trim
them to exact durations:

```powershell
powershell -ExecutionPolicy Bypass -File .\bench\make-fixtures.ps1
python .\bench\trim_fixtures.py
```

TTS clips are fine for latency and engine bake-offs. Use real recordings for
WER and multilingual quality.

## Running

Once the benchmark CLI lands:

```powershell
cargo run -p draft -- bench --fixtures bench/fixtures --runs 10 --json bench/results/local.json
```

The harness runs one warmup plus `--runs` measured passes per fixture, reports
p50/p95, and compares against `bench/baseline.json` when it exists.

## Engine bake-off

The Windows engine is `transcribe.cpp` with Vulkan. Build `transcribe-cli` and
`transcribe-bench` from a short path (for example `C:\bv`), with
`-DTRANSCRIBE_VULKAN=ON -DTRANSCRIBE_BUILD_TOOLS=ON`, then:

```powershell
C:\bv\bin\Release\transcribe-bench.exe `
  --model C:\all\models\whisper-large-v3-turbo-Q8_0.gguf `
  --sample .\bench\fixtures\tts-5s.wav `
  --backend vulkan --device 0 --warmup 3 --iters 5 `
  --quiet --json-out .\bench\results\vulkan-tts-5s.wav.json

python .\bench\compare_bakeoff.py --results-dir .\bench\results --prefix vulkan-
```

Always warm up: the first Vulkan run compiles pipelines and is not a real
latency number. Results are in [`docs/PERFORMANCE.md`](../docs/PERFORMANCE.md);
the measured Vulkan path is about 2× faster than v3.5 on short and medium clips.

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
