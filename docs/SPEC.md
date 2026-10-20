# Draft v4 — Product Spec

**Status:** pre-alpha, started 2026-10-20  
**Owner:** Tirup Mehta  
**Repo:** `draft-rs` (product name: **Draft**)

## 1. One line

Hold a key, speak naturally, and watch accurate text appear at the cursor — fully
local, faster than the cloud, with none of the clipboard gymnastics.

## 2. Why this exists

Draft v3 proved the engine: faster-whisper large-v3-turbo on a consumer GPU,
instant Flash mode, corrections, history, and a tray app that people actually
use. Hex proved the product: streaming dictation, a polished native shell,
per-app Modes, and a real release pipeline.

Draft v4 combines the two ideas and fixes what both get wrong. It is a clean
Rust implementation. Hex's source is reference material for behavior and
architecture only; no Hex code, assets, copy, or branding is used.

## 3. Product principles

1. **Local by default.** Transcription never leaves the machine. Network is for
   model downloads, updates, and an explicitly enabled rewrite provider.
2. **Streaming first.** Words appear while you speak. Release should feel like
   punctuation, not like waiting.
3. **The keyboard is sacred.** A dictation shortcut never leaks into the target
   app, never steals focus, and never eats a chord it does not own.
4. **No surprises.** The previous clipboard comes back. Failed jobs paste
   nothing. Cancelled jobs paste nothing. History is inspectable and deletable.
5. **Small and native.** No Python runtime, no bundled PyTorch. The app is a
   single native executable; models download on demand.
6. **Observable.** Every failure has a local, human-readable reason. No
   telemetry, no accounts, no silent fallbacks.

## 4. Target user

- A developer or writer who dictates prompts, emails, notes, and long-form text.
- A multilingual user who switches languages and wants names spelled correctly.
- A privacy-sensitive user who refuses cloud dictation.
- Windows 10/11 on x64 and ARM64 first; macOS and Linux later.

## 5. The "better than Hex" contract

| Area | Hex today | Draft v4 target |
| --- | --- | --- |
| Streaming | Parakeet Unified English only, GPU only | Streaming preview for every dictation; final pass with Whisper large-v3-turbo |
| Windows GPU | Vulkan, x64 only | Vulkan on x64; CPU on ARM64; optional CUDA later |
| Clipboard | Transcript owns the clipboard for ~500 ms after paste | Configurable hold (default 0–50 ms), change-count guard, direct-type fallback |
| Voice commands | macOS only | Windows-first command grammar, opt-in |
| Per-app modes | Implemented on macOS/Windows/Linux | Implemented, with a fast application picker and website rules |
| Install size | Native app + models | Native app under ~60 MB; no Python or PyTorch |
| Installer | Unsigned, SmartScreen warning | Signed installer and signed updates (when certificate is available) |
| Updates | Signed feed | Signed feed with resumable downloads and rollback |
| Platforms | macOS primary; Windows/Linux beta | Windows first-class; macOS/Linux after parity |
| Onboarding | Guided, with practice step | Guided, with a real 10-second practice dictation and live diagnostics |

## 6. Features

### MVP (M1–M3)

- Resident tray app with a native settings window.
- Hold-to-talk and toggle modes; configurable shortcut, including modifier-only
  bindings such as Right Ctrl and key chords such as Ctrl+Space.
- Microphone picker, live input level, and device fallback.
- Click-through, always-on-top recording HUD with live waveform and partial text.
- Local transcription through `transcribe.cpp` (GGUF models):
  - Parakeet Unified English for low-latency streaming.
  - Whisper large-v3-turbo for final accuracy and multilingual dictation.
  - Optional two-pass mode: stream with Parakeet, replace with the Whisper final.
- Paste at the current focus with clipboard preservation, Paste Last, and a
  direct-typing fallback for apps that block Ctrl+V.
- Model manager: download, checksum, install, select, and delete models.
- Local History with retention windows, search, copy, delete, and clear.
- Corrections, taught words, and hotwords applied before paste.
- Launch at login, optional sounds, light/dark theme, interface language later.
- Local diagnostics: NDJSON event log plus a human-readable process log.

### V1

- Per-application Modes: corrections, formatting, and transformations selected by
  the foreground application (and browser host later).
- Ordered text transformations: lowercase, sentence case, no trailing
  punctuation, custom TypeScript later.
- Optional LLM rewrite through a local OpenAI-compatible endpoint. Off by
  default; failed rewrite preserves the local transcript.
- Voice commands on Windows: "scratch that", "new paragraph", "new line",
  "send it", "select all", "press enter".
- Signed installer, silent per-user install, Start Menu entry, clean uninstall.
- Signed auto-update with version ordering, checksum verification, and rollback.
- ARM64 build.

### Later

- macOS (Metal) and Linux (Vulkan) shells.
- Meetings and system-audio capture.
- Local transcription API/SDK.
- Semantic history search.

## 7. Non-goals for v1

- Cloud transcription.
- Accounts, sync, or telemetry.
- Meeting recording.
- Mobile apps.
- A plugin marketplace.
- Rewriting the user's words without an explicit opt-in.

## 8. Architecture

```ts
Windows low-level keyboard hook
  -> Shortcut state machine (hold, toggle, double-tap lock, Escape)
  -> Capture owner (authoritative 16 kHz mono timeline, bounded ring buffer)
       |-> live audio chunks -> inference thread -> stream preview -> HUD
       |-> finished clip     -> inference thread -> final transcript
  -> Text pipeline (corrections, hotwords, transformations, optional rewrite)
  -> Output worker (single ordered queue) -> clipboard/paste -> History
```

**Ownership rules**

- The audio callback never allocates unbounded memory and never waits on
  inference, UI, paste, or disk.
- One inference thread owns the loaded model. A stream borrows the session;
  finishing a stream always falls back to the whole clip.
- The output worker is the only component that mutates the clipboard or sends
  synthetic input. Jobs complete in submission order.
- Settings, history, and model preparation are owned by the app thread and
  published as snapshots to workers.
- Every queue is bounded. Backpressure drops preview work first, then fails the
  affected job; it never corrupts an active capture.

**Module layout**

```text
src/main.rs                 CLI entry, logging, single-instance guard
src/app.rs                  app lifecycle, tray, window, listeners
src/core/settings.rs        settings.json, migrations, live projection
src/core/models.rs          catalog, download, checksum, install/delete
src/core/engine.rs          transcribe.cpp model + session owner
src/core/stream.rs          stream state, finish/fallback policy
src/core/pipeline.rs        corrections, hotwords, transformations
src/core/history.rs         bounded retained history
src/core/text.rs            pure text helpers and tests
src/platform/windows/hotkeys.rs
src/platform/windows/audio.rs
src/platform/windows/paste.rs
src/platform/windows/tray.rs
src/platform/windows/hud.rs
src/platform/windows/context.rs
src/platform/windows/login.rs
src/platform/windows/updater.rs
src/ui/                     GPUI shell, settings panes, onboarding, theme
```

**Stack**

| Concern | Choice | Why |
| --- | --- | --- |
| UI | GPUI 0.2.x | Native, GPU-rendered, proven on Windows, closest to Hex's feel |
| Audio | `cpal` | WASAPI capture, device enumeration, low latency |
| Resampling | `rubato` | High-quality 16 kHz conversion |
| Inference | `transcribe-cpp` | GGUF Parakeet/Whisper, Vulkan/CUDA/Metal, one Rust API |
| Clipboard | `arboard` + Win32 `SendInput` | Formats, change counts, direct typing |
| Tray | `tray-icon` | Native notification-area icon |
| Win32 | `windows-sys` / `windows` | Hooks, UI Automation, audio sessions, registry |
| Config | `serde` + `serde_json` | Human-readable, atomic writes |
| Logging | `tracing` | Bounded async NDJSON + human-readable log |
| Updates | `ed25519-dalek` + custom feed | Signed manifests, version ordering, rollback |

## 9. Performance budgets

- Shortcut press to capture start: **< 50 ms p95**.
- First streamed words visible: **< 400 ms** after speech onset.
- Release to pasted final text, 10 s clip, RTX 4060, Whisper turbo:
  **p50 < 500 ms**, **p95 < 1.2 s**.
- Idle RAM without a model: **< 120 MB**.
- Installer size without models: **< 60 MB**.
- No dropped audio while the UI is busy or a model is loading.

Budgets are targets, measured on the development machine first and then on a
clean Windows VM.

## 10. Data and privacy

- Runtime data lives in `%APPDATA%\Draft\` (settings, history, logs) and
  `%LOCALAPPDATA%\Draft\models\` (model files).
- Audio is never persisted by default.
- History stores text and bounded metadata only: no audio, no window titles, no
  full URLs.
- Diagnostic logs may contain transcript text; they stay local and are rotated.
- Network requests are limited to model downloads, update checks, and an
  explicitly configured rewrite provider.

## 11. Milestones

### M0 — Scaffold and proof

- Repo, license, CI, formatting, Clippy.
- Rust toolchain and Windows build prerequisites documented.
- A GPUI window opens; tray icon appears; global hook logs a chord.
- **Done when:** `cargo run` shows the window and the hook toggles a log line on
  a real Windows machine.

### M1 — Dictation MVP

- Hold Right Ctrl, record, transcribe with the selected model, paste at focus.
- Escape cancels; short taps discard; clipboard is restored.
- **Done when:** a 5-second utterance pastes correctly into Notepad, VS Code,
  and a browser, three times in a row, with the previous clipboard intact.

### M2 — Models and streaming

- Model catalog, download with progress, checksum, selection, deletion.
- Parakeet stream preview plus Whisper final pass.
- HUD shows waveform, partial text, and processing state.
- **Done when:** text appears while speaking and is replaced by the final
  transcript without flicker or double paste.

### M3 — Settings, history, corrections

- Settings for shortcut, microphone, model, language, paste, sounds, login.
- History with retention, search, copy, delete, clear.
- Corrections and hotwords applied in the pipeline.
- **Done when:** a restart preserves every setting and history entry; a taught
  correction wins on the next dictation.

### M4 — Distribution

- Inno Setup or NSIS per-user installer.
- Signed update feed, version ordering, checksum, rollback.
- Start Menu, launch at login, uninstall that keeps user data.
- **Done when:** install → update → uninstall works on a clean Windows VM.

### M5 — Modes and commands

- Foreground-app detection and per-app Modes.
- Transformations and optional rewrite.
- Voice commands.
- **Done when:** a mode for VS Code applies only in VS Code and a global mode
  applies everywhere else.

### M6 — Cross-platform

- macOS shell (Metal) and Linux shell (Vulkan) over the same core.
- **Done when:** the same settings and pipeline run on all three platforms.

## 12. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| GPUI API churn or Windows shader issues | Pin a known version; keep UI behind a small layer; fall back to egui if needed |
| GPU driver/driver-less machines | Detect Vulkan; fall back to CPU with a clear message |
| Model quality vs Draft v3 | Keep Whisper turbo as the final pass; measure WER on a fixed corpus before switching defaults |
| ARM64 performance | CPU-first; benchmark before promising GPU |
| Signing cost | Ship unsigned with clear SmartScreen guidance until a certificate exists |
| Scope creep | M0–M2 ship a complete dictation loop before any mode or meeting work |

## 13. Open decisions

- Final product name and icon.
- Whether the first release is x64-only or x64 + ARM64 together.
- Update feed host and signing key custody.
- Default shortcut (Right Ctrl vs Ctrl+Space).
- Whether two-pass streaming is on by default or an "Accuracy" setting.
