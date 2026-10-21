# Roadmap

The authoritative build order. Each milestone ends with something a user can
run, not a folder of abstractions. Desktop first, then mobile over the same
core.

## Current state (2026-10-21)

Windows x64 is the active target. The hold-to-talk loop, tray, click-through HUD,
persisted shortcut/microphone/model settings, microphone level check and device
fallback, the pinned resumable model installer, and the release Vulkan benchmark
are working. The open release gates are real-speech WER parity, a repeatable
end-to-end paste run, installer and signed updates, and the macOS/Linux/mobile
shells. Measured latency lives in [`PERFORMANCE.md`](PERFORMANCE.md).

Runtime data lives in `%LOCALAPPDATA%\Draft4` so the shipping v3.5 app's
`%LOCALAPPDATA%\Draft` folder is never read or overwritten.

## M0 — Scaffold and proof

- [x] Initialize repo, MIT license, `.gitignore`, CI.
- [x] Write the product spec and architecture.
- [x] Split the platform-neutral core from the CLI.
- [x] Install Rust, VS Build Tools, and the Windows SDK.
- [x] Install the Vulkan SDK.
- [x] Set `GPUI_FXC_PATH` to the Windows SDK shader compiler.
- [x] Open a GPUI window with the Draft theme.
- [x] Add a tray icon with Show, Hide, Paste last, and Quit; closing the window
      hides it to the tray.
- [x] Register a low-level keyboard hook and log a Right Ctrl hold. Six bindings
      ship: Right Ctrl, Right Alt, F8, F9, Pause, and Scroll Lock.
- [x] `cargo fmt --check`, `cargo clippy -D warnings`, and
      `cargo test --workspace` run in CI.

## M1 — Dictation MVP

- [x] Capture the Draft v3.5 speed baseline on the reference RTX 4060, using
      the same clips and the same `large-v3-turbo` model.
- [x] Engine bake-off: `transcribe.cpp` Vulkan selected. Warm `large-v3-turbo`
      Q8_0 is 2.9-4.1x faster than v3.5; CUDA was not pursued, and the
      CTranslate2 shim stays a fallback only if the WER gate fails.
- [x] `bench/baseline.json` records the v3.5 baseline.
- [x] `draftctl bench --json` and the 10% p95 gate. Release p95 is 246-1,043 ms
      on the 3-60 second clips, clearing the 1.10x ceiling.
- [x] `cpal` capture at the device rate, downmix to mono, resample to 16 kHz,
      and persist the selected device.
- [x] Shortcut state machine: hold, 300 ms short-tap discard, and Escape cancel.
- [x] Load a local GGUF model through `transcribe-cpp` and keep it warm.
- [x] Whole-clip transcription and paste with clipboard restoration.
- [x] Paste Last from the tray.
- [ ] Direct-typing fallback for applications that block the paste chord.
- [x] Engine fixture: the original 11-second JFK WAV transcribes exactly.
- [ ] End-to-end paste into Notepad, VS Code, and a browser, three times each,
      with the previous clipboard intact.
- [x] **Latency acceptance:** final p95 is no worse than v3.5; target 20% faster
      is met by a wide margin.
- [ ] **Accuracy acceptance:** real-speech WER is within 0.5 percentage points of
      v3.5. This is the remaining release gate.

## M2 — Models and streaming

- [ ] Model catalog with Parakeet Unified English and Whisper large-v3-turbo.
- [x] Pinned Whisper Q8_0 artifact (length and SHA-256), resumable download with
      progress, verification, atomic install, status, verify, and remove.
- [ ] Parakeet Unified English entry in the same catalog.
- [ ] Streaming session with partial text; final whole-clip fallback.
- [x] HUD: click-through, always-on-top listening and transcribing states.
- [ ] HUD waveform and partial text.
- [ ] Two-pass mode: Parakeet preview, Whisper final.
- [ ] Benchmark F16 vs Q8_0 on real speech; Q8_0 is selected from published WER,
      with F16 as the fallback.
- [ ] Prove the preview model does not regress final p95 by more than 2%.

## M3 — Settings, history, corrections

- [x] Settings window card and `settings.json` with atomic writes.
- [x] Persist the shortcut, microphone, and model path; restore them at launch.
- [x] Microphone picker, four-second live level check, and device fallback.
- [ ] Shortcut capture UI for arbitrary keys and chords. Six presets ship now.
- [ ] Hold/toggle modes, double-tap lock, and a toggle shortcut.
- [ ] History with retention, search, copy, delete, clear.
- [x] History store: text and duration only, 30-day default window, 2,000
      entries, 8 MB cap, case-insensitive search, delete, clear, and a
      quarantine for a malformed file. Exposed through `draftctl history`.
- [x] Record finished dictations into history from the app thread; a failed
      write never blocks dictation.
- [ ] History pane with search, copy, delete, clear, and retention controls.
- [ ] Corrections, taught words, hotwords.
- [ ] Launch at login, sounds, theme.

## M4 — Distribution

- [ ] Per-user installer with Start Menu / Applications entry and clean uninstall.
- [ ] Signed update feed with version ordering and rollback.
- [ ] Clean-VM install -> update -> uninstall test.
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
