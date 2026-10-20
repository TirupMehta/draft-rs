# Benchmarks

Performance is a release gate. The reference machine, the Draft v3.5 baseline,
and the pass/fail rule live in [`docs/PERFORMANCE.md`](../docs/PERFORMANCE.md).

## Fixtures

Audio is not committed: it is large, personal, and easy to regenerate. Put
16 kHz mono WAVs in `bench/fixtures/` (gitignored):

| File | Content |
| --- | --- |
| `short-3s.wav` | Greeting, one sentence |
| `short-5s.wav` | Name, comma, question mark |
| `medium-15s.wav` | Several sentences, natural pauses |
| `long-30s.wav` | Paragraph with punctuation and a list |
| `long-60s.wav` | Sustained speech, no long silence |
| `multilingual-10s.wav` | Non-English sentence with a proper noun |

Record all fixtures with the same microphone and room for a fair comparison.
Do not commit personal audio.

## Running

Once the benchmark CLI lands:

```powershell
cargo run -p draft -- bench --fixtures bench/fixtures --runs 10 --json bench/results/local.json
```

The harness runs one warmup plus `--runs` measured passes per fixture, reports
p50/p95, and compares against `bench/baseline.json` when it exists.

## Baseline schema

`bench/baseline.json` is committed only after a real measurement on the
reference machine. No placeholder numbers.

```json
{
  "created": "2026-10-20T00:00:00Z",
  "machine": {
    "cpu": "Intel Core i7-13620H",
    "gpu": "NVIDIA GeForce RTX 4060 Laptop 8 GB",
    "ram_gb": 16,
    "os": "Windows 11 build 26300"
  },
  "engine": {
    "name": "draft-v3.5",
    "model": "large-v3-turbo",
    "backend": "cuda",
    "compute_type": "float16",
    "beam_size": 5
  },
  "fixtures": {
    "short-5s.wav": {
      "final_ms": { "p50": 0, "p95": 0 },
      "paste_ms": { "p50": 0, "p95": 0 },
      "first_partial_ms": { "p50": 0, "p95": 0 },
      "rtf": 0
    }
  },
  "wer": {
    "fixture-set": 0
  }
}
```
