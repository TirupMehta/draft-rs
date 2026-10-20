# Roadmap

The authoritative build order. Each milestone ends with something a user can
run, not a folder of abstractions. Desktop first, then mobile over the same
core.

## M0 — Scaffold and proof

- [x] Initialize repo, MIT license, `.gitignore`, CI.
- [x] Write the product spec and architecture.
- [x] Split the platform-neutral core from the CLI.
- [ ] Install Rust, VS Build Tools, Windows SDK, Vulkan SDK.
- [ ] Open a GPUI window with the Draft theme.
- [ ] Add a tray icon with Show, Hide, Quit.
- [ ] Register a low-level keyboard hook and log a Right Ctrl hold.
- [ ] `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`
      in CI.

## M1 — Dictation MVP

- [ ] `cpal` capture at 16 kHz mono with device selection.
- [ ] Shortcut state machine: hold, short-tap discard, Escape cancel.
- [ ] Load a local GGUF model through `transcribe-cpp`.
- [ ] Whole-clip transcription and paste with clipboard restoration.
- [ ] Paste Last and a direct-typing fallback.
- [ ] End-to-end test: 5-second WAV fixture transcribes to expected text.

## M2 — Models and streaming

- [ ] Model catalog with Parakeet Unified English and Whisper large-v3-turbo.
- [ ] Download with progress, SHA-256 verification, atomic install.
- [ ] Streaming session with partial text; final whole-clip fallback.
- [ ] HUD: waveform, partial text, processing state, click-through.
- [ ] Two-pass mode: Parakeet preview, Whisper final.

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
