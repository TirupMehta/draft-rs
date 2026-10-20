# Roadmap

The authoritative build order. Each milestone ends with something a user can
run, not a folder of abstractions. Desktop first, then mobile over the same
core.

## M0 — Scaffold and proof

- [x] Initialize repo, MIT license, `.gitignore`, CI.
- [x] Write the product spec and architecture.
- [x] Split the platform-neutral core from the CLI.
- [x] Install Rust, VS Build Tools, and the Windows SDK.
- [x] Install the Vulkan SDK.
- [x] Set `GPUI_FXC_PATH` to the Windows SDK shader compiler.
- [x] Open a GPUI window with the Draft theme.
- [ ] Add a tray icon with Show, Hide, Quit.
- [ ] Register a low-level keyboard hook and log a Right Ctrl hold.
- [ ] `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`
      in CI.

## M1 — Dictation MVP

- [x] Capture the Draft v3.5 speed baseline on the reference RTX 4060, using
      the same clips and the same `large-v3-turbo` model.
- [x] Engine bake-off: `transcribe.cpp` Vulkan selected. Warm `large-v3-turbo`
      Q8_0 is ~2× faster than v3.5; CUDA was not pursued, and the CTranslate2
      shim stays a fallback only if the WER gate fails.
- [x] `bench/baseline.json` records the v3.5 baseline.
- [ ] `draft bench --json` and the 10% p95 gate.
- [ ] `cpal` capture at 16 kHz mono with device selection.
- [ ] Shortcut state machine: hold, short-tap discard, Escape cancel.
- [ ] Load a local GGUF model through `transcribe-cpp`.
- [ ] Whole-clip transcription and paste with clipboard restoration.
- [ ] Paste Last and a direct-typing fallback.
- [ ] End-to-end test: 5-second WAV fixture transcribes to expected text.
- [ ] **Acceptance:** final p95 is no worse than v3.5; target 20% faster.

## M2 — Models and streaming

- [ ] Model catalog with Parakeet Unified English and Whisper large-v3-turbo.
- [ ] Download with progress, SHA-256 verification, atomic install.
- [ ] Streaming session with partial text; final whole-clip fallback.
- [ ] HUD: waveform, partial text, processing state, click-through.
- [ ] Two-pass mode: Parakeet preview, Whisper final.
- [ ] Benchmark F16 vs Q8_0; pick on WER first, latency second.
- [ ] Prove the preview model does not regress final p95 by more than 2%.

## M3 — Settings, history, corrections

- [ ] Settings window and `settings.json` with atomic writes.
- [ ] Microphone picker, live level, device fallback.
- [ ] Shortcut capture UI, hold/toggle, double-tap lock.
- [ ] History with retention, search, copy, delete, clear.
- [ ] Corrections, taught words, hotwords.
- [ ] Launch at login, sounds, theme.

## M4 — Distribution

- [ ] Per-user installer with Start Menu / Applications entry and clean uninstall.
- [ ] Signed update feed with version ordering and rollback.
- [ ] Clean-VM install → update → uninstall test.
- [ ] Code signing when a certificate is available.

## M5 — Modes and commands

- [ ] Foreground application detection on every desktop OS.
- [ ] Per-app modes with corrections and transformations.
- [ ] Optional local/OpenAI-compatible rewrite.
- [ ] Voice commands: scratch that, new paragraph, send, select all.

## M6 — Desktop everywhere

- [ ] macOS shell and Metal inference.
- [ ] Linux shell (X11 + Wayland) and Vulkan inference.
- [ ] One settings schema and history store across all three.
- [ ] Platform permission and device-recovery parity.

## M7 — Mobile

- [ ] Stable C ABI for `draft-core`.
- [ ] iOS shell (SwiftUI) with a keyboard extension and background limits.
- [ ] Android shell (Compose) with an IME and foreground service.
- [ ] Model download and storage policy per platform.
