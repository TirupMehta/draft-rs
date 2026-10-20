# Draft v4 — Product Spec

**Status:** pre-alpha, started 2026-10-20  
**Owner:** Tirup Mehta  
**Repo:** `draft-rs` (product name: **Draft**)

## 1. One line

Hold a key, speak naturally, and watch accurate text appear at the cursor — on
every device, fully local, faster than the cloud, with none of the clipboard
gymnastics.

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
- A user who moves between a Windows PC, a Mac, a Linux workstation, and a
  phone without changing how dictation works.

Desktop is the first target: Windows 10/11 on x64 and ARM64, macOS on Apple
silicon, and Linux on X11 and compatible Wayland compositors. Mobile is a later
shell over the same core, not a separate product.

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
| Platforms | macOS primary; Windows/Linux beta | Windows, macOS, and Linux first-class; iOS/Android shells later |
| Onboarding | Guided, with practice step | Guided, with a real 10-second practice dictation and live diagnostics |

## 6. Features

### MVP (M1–M3)

- Resident desktop app with a native settings window and menu-bar/tray icon.
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
- Voice commands on desktop: "scratch that", "new paragraph", "new line",
  "send it", "select all", "press enter".
- Signed installer, silent per-user install, Start Menu entry, clean uninstall.
- Signed auto-update with version ordering, checksum verification, and rollback.
- ARM64 build.

### Later

- macOS (Metal) and Linux (Vulkan) desktop shells.
- iOS and Android shells over the same core, with native keyboards and
  platform-appropriate background limits.
- Meetings and system-audio capture.
- Local transcription API/SDK.
- Semantic history search.

## 7. Non-goals for v1

- Cloud transcription.
- Accounts, sync, or telemetry.
- Meeting recording.
- Mobile apps (M7, after the desktop core is stable).
- A plugin marketplace.
- Rewriting the user's words without an explicit opt-in.

## 8. Architecture

```ts
Platform input hook (Win32 hook / CGEventTap / evdev)
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
crates/draft-core/src/lib.rs        platform-neutral facade
crates/draft-core/src/text.rs       pure text helpers and tests
crates/draft-core/src/settings.rs   settings schema, migrations, live projection
crates/draft-core/src/models.rs     catalog, download, checksum, install/delete
crates/draft-core/src/engine.rs     transcribe.cpp model + session owner
crates/draft-core/src/stream.rs     stream state, finish/fallback policy
crates/draft-core/src/pipeline.rs   corrections, hotwords, transformations
crates/draft-core/src/history.rs    bounded retained history
crates/draft-core/src/api.rs        stable C ABI for mobile shells (M7)
crates/draft-cli/src/main.rs        development CLI
apps/desktop/src/main.rs            GPUI shell entry and lifecycle
apps/desktop/src/theme.rs           design tokens and typography
apps/desktop/src/panes/             settings, history, models, onboarding
apps/desktop/src/platform/windows/  hook, WASAPI, paste, tray, HUD, UIA, registry
apps/desktop/src/platform/macos/    event tap, CoreAudio, paste, menu bar, Metal
apps/desktop/src/platform/linux/    evdev/X11, PipeWire/ALSA, paste, layer-shell HUD
apps/mobile/ios/                    SwiftUI shell over the C ABI (M7)
apps/mobile/android/                Compose shell over the C ABI (M7)
```

**Stack**

| Concern | Choice | Why |
| --- | --- | --- |
| Desktop UI | GPUI 0.2.x | Native, GPU-rendered, one shell for Windows, macOS, and Linux |
| Mobile UI | SwiftUI + Jetpack Compose | Native keyboards and platform background rules; shares the Rust core |
| Audio | `cpal` | WASAPI, CoreAudio, ALSA/PipeWire capture and device enumeration |
| Resampling | `rubato` | High-quality 16 kHz conversion |
| Inference | `transcribe-cpp` | GGUF Parakeet/Whisper, Vulkan/CUDA/Metal/CPU, one Rust API |
| Clipboard | `arboard` + native synthetic input | Formats, change counts, direct typing |
| Tray | `tray-icon` + native menu-bar adapters | Native notification-area and menu-bar integration |
| Platform | `windows-sys`, `objc2`, `x11rb`/Wayland | Hooks, UI Automation, event taps, audio sessions, registry |
| Config | `serde` + `serde_json` | Human-readable, atomic writes |
| Logging | `tracing` | Bounded async NDJSON + human-readable log |
| Updates | `ed25519-dalek` + custom feed | Signed manifests, version ordering, rollback |
| Mobile bridge | C ABI + generated bindings | Keeps `draft-core` free of UI and OS dependencies |

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

- Runtime data lives in the platform application-data directory:
  `%APPDATA%\Draft\` on Windows, `~/Library/Application Support/Draft/` on
  macOS, and `$XDG_DATA_HOME/draft/` (or `~/.local/share/draft/`) on Linux.
  Model files live in the platform cache directory (`%LOCALAPPDATA%\Draft\models`
  on Windows) and are re-downloadable.
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
- **Done when:** `cargo run -p draft` shows the window and the hook toggles a log
  line on a real Windows machine. macOS and Linux are proven in M6.

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

- Windows per-user installer (Inno Setup or NSIS) with Start Menu, launch at
  login, and uninstall that keeps user data.
- Signed update feed, version ordering, checksum, rollback.
- macOS and Linux packaging follow their platform rules in M6.
- **Done when:** install → update → uninstall works on a clean Windows VM.

### M5 — Modes and commands

- Foreground-app detection and per-app Modes.
- Transformations and optional rewrite.
- Voice commands.
- **Done when:** a mode for VS Code applies only in VS Code and a global mode
  applies everywhere else.

### M6 — Desktop everywhere

- macOS shell (Metal) and Linux shell (Vulkan) over the same core.
- Per-platform permissions, hotkeys, audio devices, paste, tray/menu bar, and
  installers.
- **Done when:** the same settings and pipeline run on all three desktop
  platforms and a dictation started on one platform has the same behavior on the
  others.

### M7 — Mobile

- Stable C ABI for `draft-core` plus generated Swift and Kotlin bindings.
- iOS keyboard extension and Android IME.
- Platform background limits, model storage, and memory budgets.
- **Done when:** dictation works in a third-party app on a physical iPhone and a
  physical Android phone without a cloud round trip.

## 12. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| GPUI API churn or Windows shader issues | Pin a known version; keep UI behind a small layer; fall back to egui if needed |
| GPU driver/driver-less machines | Detect Vulkan; fall back to CPU with a clear message |
| Model quality vs Draft v3 | Keep Whisper turbo as the final pass; measure WER on a fixed corpus before switching defaults |
| ARM64 performance | CPU-first; benchmark before promising GPU |
| Signing cost | Ship unsigned with clear SmartScreen guidance until a certificate exists |
| Mobile background limits | Keep mobile in M7; design the core around explicit start/stop and a bounded capture lease |
| Scope creep | M0–M2 ship a complete dictation loop before any mode or meeting work |

## 13. Open decisions

- Final product name and icon.
- Whether the first release is x64-only or x64 + ARM64 together.
- Whether iOS or Android ships first in M7.
- Update feed host and signing key custody.
- Default shortcut (Right Ctrl vs Ctrl+Space).
- Whether two-pass streaming is on by default or an "Accuracy" setting.
