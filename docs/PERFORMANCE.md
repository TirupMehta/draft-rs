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
| Partial decode | beam 1, VAD off, last ~20 s |
| Paste | direct typing |

The harness reproduces these settings on the same 16 kHz clips. A Rust CPU
build is not compared with a Python CUDA build and called a regression; the
engine bake-off below decides the backend.

Measured on 2026-10-20 with the 16 kHz TTS fixtures, ten warm runs each:

| Clip | v3.5 p50 | v3.5 p95 | 20% target p50 | 20% target p95 |
| --- | --- | --- | --- | --- |
| 3 s | 943 ms | 996 ms | 755 ms | 797 ms |
| 5 s | 880 ms | 929 ms | 704 ms | 743 ms |
| 15 s | 1,077 ms | 1,139 ms | 862 ms | 911 ms |
| 30 s | 1,473 ms | 1,601 ms | 1,179 ms | 1,281 ms |
| 60 s | 2,900 ms | 3,036 ms | 2,320 ms | 2,428 ms |

These cover decode only, not paste. The short clips carry roughly 800 ms of
fixed overhead while the 60-second clip runs at 20.7x real time. The first
speed win is removing that fixed cost, not changing the model. The measured
numbers live in [`../bench/baseline.json`](../bench/baseline.json).

## Release result (2026-10-20)

The first authoritative Rust result uses the release profile
(`cargo build --release`), `whisper-large-v3-turbo-Q8_0.gguf`, the Vulkan
backend, one warm-up plus ten measured runs per clip. Model load was 2.7 s and
the warm-up pass was 268-337 ms. These are decode times, before clipboard
paste. The raw report is [`../bench/results/rust-vulkan-release.json`](../bench/results/rust-vulkan-release.json).

| Clip | v3.5 p50 | v3.5 p95 | Draft p50 | Draft p95 | Speedup p50 | Speedup p95 |
| --- | --- | --- | --- | --- | --- | --- |
| 3 s | 943 ms | 996 ms | **229 ms** | **246 ms** | **4.12x** | **4.05x** |
| 5 s | 880 ms | 929 ms | **239 ms** | **272 ms** | **3.68x** | **3.42x** |
| 15 s | 1,077 ms | 1,139 ms | **331 ms** | **348 ms** | **3.26x** | **3.28x** |
| 30 s | 1,473 ms | 1,601 ms | **437 ms** | **477 ms** | **3.37x** | **3.36x** |
| 60 s | 2,900 ms | 3,036 ms | **951 ms** | **1,043 ms** | **3.05x** | **2.91x** |

The release build clears the 1.10x p95 ceiling and the 0.80x stretch target on
every clip by a wide margin. The earlier debug build was about 1.5-2x slower;
those numbers are not authoritative. The 60-second debug outlier (4.1 s p95)
did not reproduce in release, where p95 is 1.04 s.

A real 11-second JFK recording decoded exactly at 311 ms p50 / 315 ms p95
(35x real time). That is a sanity check, not a WER set.

What the numbers mean:

- The fixed ~800 ms overhead in v3.5 is gone. Short clips are dominated by the
  ~170-200 ms Whisper encoder pass, not setup.
- Decode is only ~40-85 ms for a 3-11 second clip; host BLAS would not move the
  number, so Windows does not need OpenBLAS.
- Q8_0 was selected over F16: the publisher's evaluation reports 2.01% WER for
  both, while Q8_0 is 886 MB versus 1.63 GB. F16 remains the fallback if real
  speech shows a Q8_0 regression.
- The first Vulkan run is much slower because it compiles compute pipelines.
  Warm-up is mandatory: the app prewarms the backend and model at launch,
  before the first dictation.
- `transcribe.cpp` Whisper currently uses greedy decoding, while v3.5 used
  beam 5 plus VAD. The latency win is real, but it is not yet an accuracy win.
  WER on real recordings must be within 0.5 percentage points of v3.5 before
  this engine ships. If it fails, test F16, then a beam/best-of path, then the
  CTranslate2 fallback. Do not hide an accuracy regression behind a speed
  table.

## Contract

| Stage | Budget | Notes |
| --- | --- | --- |
| Keydown -> capture active | <= 30 ms p95 warm, <= 300 ms p95 cold | Cold is release-while-idle or a sleeping device |
| Speech onset -> first partial | <= 300 ms p95, stretch 150 ms | Preview model only; never blocks the final pass |
| Keyup -> final text ready | <= v3.5 p50 **and** p95 on the same clip | Stretch: 20% faster than v3.5 |
| Final text ready -> paste issued | <= 40 ms p95 | Clipboard + `SendInput` |
| Keyup -> paste complete | final + <= 80 ms p95 | End-to-end number users feel |
| App launch -> dictation ready | <= 3 s warm cache | Model is memory-mapped, not copied |
| Idle CPU | <= 0.5% | No polling loops in the hot path |
| Working set | <= 300 MB | Model weights live in VRAM/page cache |
| VRAM | <= 6.5 GB | Leaves headroom on an 8 GB laptop GPU |
| Audio integrity | zero drops | 60 s capture with UI, paste, and a model load busy |

The merge gate is the measured v3.5 p50 and p95 for each clip. These absolute
ceilings are the stretch once the fixed overhead is gone; they are not a
substitute for the gate:

| Clip | Release -> paste p50 | p95 |
| --- | --- | --- |
| 3 s | 300 ms | 600 ms |
| 5 s | 350 ms | 700 ms |
| 15 s | 500 ms | 900 ms |
| 30 s | 800 ms | 1.3 s |
| 60 s | 1.8 s | 2.5 s |

## Model policy

- The default final model is **`whisper-large-v3-turbo`**, the same family as
  v3.5. It is not silently replaced by a smaller model to hit a budget.
- Q8_0 is the current distribution artifact. F16 is the fallback if real-speech
  WER shows that quantization costs more than 0.5 percentage points.
- Parakeet and Moonshine may power live preview. They replace the final
  Whisper pass only after matching its WER on our fixtures and beating its p95.
- Multilingual behavior is part of correctness; an English-only speed win is
  not a free trade.

## Engine bake-off

| Candidate | Why | Status |
| --- | --- | --- |
| `transcribe.cpp` Vulkan | Windows GPU path with no CUDA toolkit for users | **Selected for Windows**: release build is 2.9-4.1x faster than v3.5 |
| `transcribe.cpp` CUDA | Native NVIDIA path if the Windows build is sound | Not pursued: upstream documents CUDA as Linux-only |
| CTranslate2 C++ shim | Exact v3.5 engine; highest chance of parity | Fallback only if Vulkan WER fails |
| whisper.cpp CUDA | Mature Windows CUDA path | Not needed: Vulkan already beats v3.5 |

Decision rule: choose the fastest p95 that meets the WER bar, then weigh
installer size and user setup cost. Vulkan won the latency benchmark; WER on
real recordings is still the release gate.

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
- `draftctl bench --json` writes results; `bench/baseline.json` is the
  committed reference.
- Gate: p95 must be <= 1.10x baseline and WER <= baseline + 0.5 percentage
  points. Target: p95 <= 0.80x baseline.

## Regression rules

- Pull requests that touch capture, inference, paste, or model loading run
  `draftctl bench` on the reference machine.
- CI runs correctness; the performance gate is a release checklist item.
- Any p95 regression above 10% blocks a release until it is fixed or the
  baseline is deliberately updated with a recorded reason.
- A baseline update is a product decision, never a quiet test fix.

## Status

The v3.5 baseline, the `transcribe.cpp` Vulkan engine, and the Rust benchmark
harness are recorded. Release latency is well inside the contract. The
remaining release gate is WER on real speech within 0.5 percentage points of
v3.5; it has not been measured yet.
