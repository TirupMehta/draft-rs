# Performance contract

Draft v3.5 is the speed baseline. It feels fast because it keeps
`large-v3-turbo` warm on the RTX 4060 and decodes with CTranslate2 CUDA. The
Rust rewrite must not trade that away for portability or a smaller model.

**Speed is a release criterion, not a benchmark appendix.**

## Reference machine

| | |
| --- | --- |
| CPU | Intel Core i7-13620H, 10 cores / 16 threads |
| GPU | NVIDIA GeForce RTX 4060 Laptop, 8 GB |
| RAM | 16 GB |
| OS | Windows 11 build 26300 |

Windows latency is measured here first. A clean VM without a GPU records the
CPU fallback separately; it is never compared with the CUDA baseline.

## Draft v3.5 baseline (the thing to beat)

| | |
| --- | --- |
| Engine | faster-whisper 1.2.1 / CTranslate2 4.8.2 |
| Model | `large-v3-turbo` |
| Device | CUDA, `float16` |
| Final decode | beam 5, VAD on, temperature 0, no previous-text conditioning |
| Partial decode | beam 1, VAD off, last ≤ 20 s |
| Paste | direct typing |

The harness reproduces these settings on the same 16 kHz clips. A Rust CPU
build is not compared with a Python CUDA build and called a regression; the
engine bake-off below decides the backend.

## Contract

| Stage | Budget | Notes |
| --- | --- | --- |
| Keydown → capture active | ≤ 30 ms p95 warm, ≤ 300 ms p95 cold | Cold is release-while-idle or a sleeping device |
| Speech onset → first partial | ≤ 300 ms p95, stretch 150 ms | Preview model only; never blocks the final pass |
| Keyup → final text ready | ≤ v3.5 p50 **and** p95 on the same clip | Stretch: 20% faster than v3.5 |
| Final text ready → paste issued | ≤ 40 ms p95 | Clipboard + `SendInput` |
| Keyup → paste complete | final + ≤ 80 ms p95 | End-to-end number users feel |
| App launch → dictation ready | ≤ 3 s warm cache | Model is memory-mapped, not copied |
| Idle CPU | ≤ 0.5% | No polling loops in the hot path |
| Working set | ≤ 300 MB | Model weights live in VRAM/page cache |
| VRAM | ≤ 6.5 GB | Leaves headroom on an 8 GB laptop GPU |
| Audio integrity | zero drops | 60 s capture with UI, paste, and a model load busy |

Absolute ceilings until the baseline is recorded — these are upper bounds, not
targets. If v3.5 is faster, v3.5 wins:

| Clip | Release → paste p50 | p95 |
| --- | --- | --- |
| 5 s | 350 ms | 700 ms |
| 15 s | 700 ms | 1.4 s |
| 30 s | 1.2 s | 2.2 s |

## Model policy

- The default final model is **`whisper-large-v3-turbo`**, the same family as
  v3.5. It is not silently replaced by a smaller model to hit a budget.
- F16 is the default on the 8 GB reference GPU. Q8_0 or Q5_K_M is allowed only
  when measured WER is unchanged and the latency win is real.
- Parakeet and Moonshine may power live preview. They replace the final
  Whisper pass only after matching its WER on our fixtures and beating its p95.
- Multilingual behavior is part of correctness; an English-only speed win is
  not a free trade.

## Engine bake-off

| Candidate | Why | Status |
| --- | --- | --- |
| `transcribe.cpp` Vulkan | Windows GPU path with no CUDA toolkit for users | Default candidate |
| `transcribe.cpp` CUDA | Native NVIDIA path if the Windows build is sound | Evaluate |
| CTranslate2 C++ shim | Exact v3.5 engine; highest chance of parity | Fallback if Vulkan loses |
| whisper.cpp CUDA | Mature Windows CUDA path | Alternative |

Decision rule: choose the fastest p95 that meets the WER bar, then weigh
installer size and user setup cost. Do not assume Vulkan is fast enough.
Measure it against the CUDA baseline before committing.

Windows CUDA support in `transcribe.cpp` is documented as Linux-only in its
CMake options. Treat it as unproven until a Windows build and a benchmark both
pass.

## Why Rust should win

- No Python interpreter or PyInstaller startup.
- The model is mapped once and stays warm; no per-clip construction.
- No GIL, Tk event loop, or Python-side decode lock.
- The final decode starts at keyup, before any overlay, history, or clipboard
  work finishes.
- Live preview runs on a separate small model and never queues ahead of the
  final pass.
- One resample to 16 kHz, one clipboard write, direct `SendInput`.
- No VAD pass on short clips; the capture energy gate decides instead.
- GPU shaders are warmed at startup, not on the first dictation.

## Benchmark protocol

- Fixtures: 3 s, 5 s, 15 s, 30 s, and 60 s, 16 kHz mono, with speech, silence,
  punctuation, names, and a multilingual case.
- One warmup plus ten measured runs per fixture.
- Report p50, p95, real-time factor, first-partial latency, final latency, and
  paste latency.
- Record cold start and warm steady state separately.
- `draft bench --json` writes results; `bench/baseline.json` is the committed
  reference.
- Gate: p95 must be ≤ 1.10× baseline and WER ≤ baseline + 0.5 percentage
  points. Target: p95 ≤ 0.80× baseline.

## Regression rules

- Pull requests that touch capture, inference, paste, or model loading run
  `draft bench` on the reference machine.
- CI runs correctness; the performance gate is a release checklist item.
- Any p95 regression above 10% blocks a release until it is fixed or the
  baseline is deliberately updated with a recorded reason.
- A baseline update is a product decision, never a quiet test fix.

## Status

The v3.5 baseline capture, the engine bake-off, and `draft bench` are M1 work.
No numbers are claimed here until they are written to `bench/baseline.json`.
