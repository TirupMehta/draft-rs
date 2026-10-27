# Draft v4 - Product Spec

**Status:** pre-alpha, started 2026-10-20  
**Owner:** Tirup Mehta  
**Repo:** `draft-rs` (product name: **Draft**)

## 1. One line

Hold a key, speak naturally, and watch accurate text appear at the cursor - on
every device, fully local, faster than the cloud, with none of the clipboard
gymnastics.

**Current build (Windows x64, 2026-10-21).** Hold-to-talk dictation, six
shortcut presets, the tray and click-through HUD with a live waveform and
rolling partial text, persisted shortcut/microphone/model settings, the
microphone level check and fallback, the pinned resumable model installer, the
bounded history store, the Vulkan benchmark, and the per-user Inno Setup wizard
are working. The native Parakeet streaming pass, the history pane, signed
updates, macOS/Linux shells, and mobile are not built yet. Real-speech WER
parity is the open release gate. [`ROADMAP.md`](ROADMAP.md) is the live
checklist.

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
   model downloads and updates, and nothing else.
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
| Windows GPU | Vulkan, x64 only | Vulkan x64; CPU on ARM64; CUDA not pursued on Windows (upstream Linux-only) |
| Clipboard | Transcript owns the clipboard for ~500 ms after paste | Configurable hold (default 0-50 ms), change-count guard, direct-type fallback |
| Voice commands | macOS only | Windows-first command grammar, opt-in |
| Per-app modes | Implemented on macOS/Windows/Linux | Implemented, with a fast application picker and website rules |
| Install size | Native app + models | Native app under ~60 MB; no Python or PyTorch |
| Installer | Unsigned, SmartScreen warning | Signed installer and signed updates (when certificate is available) |
| Updates | Signed feed | Signed feed with resumable downloads and rollback |
| Platforms | macOS primary; Windows/Linux beta | Windows, macOS, and Linux first-class; iOS/Android shells later |
| Onboarding | Guided, with practice step | Guided, with a real 10-second practice dictation and live diagnostics |

## 6. Features

### MVP (M1-M3)

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
- Deterministic formatting of a finished transcript: paragraph breaks at real
  pauses, measured against the speaker's own learned rhythm. No rewrite, no
  reordering, no added or dropped words.
- Spoken commands that must be their own utterance, with real silence on both
  sides: "new paragraph", "new line", and "scratch that".
- Launch at login (Windows tray-first), optional sounds, light/dark theme,
  interface language later.
- Local diagnostics: NDJSON event log plus a human-readable process log.

### V1

- Per-application Modes: corrections, formatting, and transformations selected by
  the foreground application (and browser host later).
- Ordered text transformations: lowercase, sentence case, no trailing
  punctuation, custom TypeScript later.
- More voice commands on desktop: "send it", "select all", "press enter".
- Opt-in lists: numbered and bulleted, from explicit enumeration or a spoken
  command (0.3.1).
- Signed installer, silent per-user install, Start Menu entry, clean uninstall.
- Signed auto-update with version ordering, checksum verification, and rollback.
- ARM64 build.

### Rejected

- **LLM rewrite.** An optional rewrite pass through a local OpenAI-compatible
  endpoint was planned. It is rejected: a rewrite can change a word the speaker
  said, and the formatting contract is that it never does. Structure comes from
  pauses and explicit commands, both of which are measurable and reversible.

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
  -> Text pipeline (corrections, hotwords, deterministic formatting)
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
crates/draft-core/src/text.rs       pure text helpers and meaningful-text checks
crates/draft-core/src/settings.rs   schema, atomic persistence, forward-compatible fields
crates/draft-core/src/paths.rs      the Draft4 per-user data and model folders
crates/draft-core/src/models.rs     pinned catalog, verified/resumable install, delete
crates/draft-core/src/history.rs    bounded retained history, search, delete, clear
crates/draft-engine/src/lib.rs      transcribe.cpp model and warm session owner
crates/draft-audio/src/lib.rs       cpal capture, device selection, level, 16 kHz resample
crates/draft-hotkey/src/lib.rs      cross-platform hotkey vocabulary and events
crates/draft-hotkey/src/windows.rs  low-level keyboard hook
crates/draft-paste/src/windows.rs   clipboard-preserving paste
crates/draft-cli/src/main.rs        draftctl development CLI
apps/desktop/src/main.rs            GPUI entry, options, window and HUD creation
apps/desktop/src/app.rs             shared window, Preferences, phases, tray wiring
apps/desktop/src/worker.rs          model worker, transcription and paste queue
apps/desktop/src/hud.rs             click-through recording/transcribing HUD
apps/desktop/src/tray.rs            tray icon and menu
apps/desktop/src/window_control.rs  native show/hide and HUD window styles
apps/desktop/src/theme.rs           design tokens and typography
```

Planned, not present yet:

```text
crates/draft-core/src/stream.rs     stream state, finish/fallback policy
crates/draft-core/src/pipeline.rs   corrections, hotwords, transformations
crates/draft-core/src/api.rs        stable C ABI for mobile shells (M7)
apps/desktop/src/platform/...       macOS/Linux hooks, audio, paste, HUD
apps/mobile/...                     SwiftUI/Compose shells over the C ABI (M7)
```

**Stack**

| Concern | Choice | Why |
| --- | --- | --- |
| Desktop UI | GPUI 0.2.2 (pinned) | Native, GPU-rendered, one shell for Windows, macOS, and Linux |
| Mobile UI | SwiftUI + Jetpack Compose | Native keyboards and platform background rules; shares the Rust core |
| Audio | `cpal` | WASAPI, CoreAudio, ALSA/PipeWire capture and device enumeration |
| Resampling | `rubato` | High-quality 16 kHz conversion |
| Inference | `transcribe-cpp` 0.4 | GGUF Whisper large-v3-turbo; Vulkan on Windows (2.5-4.2x faster than v3.5 on the release clips); Metal/CUDA wiring pending, and CUDA is Linux-only upstream |
| Default model | `whisper-large-v3-turbo-F16.gguf` | Same model and float16 precision as Draft v3.5; 1.63 GB; Q8_0 is a smaller catalog alternative |
| Clipboard | `arboard` + native synthetic input | Clipboard preservation plus the platform paste chord |
| Tray | `tray-icon` 0.24 | Native notification-area integration; macOS/Linux menu-bar adapters later |
| Platform | `windows-sys`; `objc2`, `x11rb`/Wayland later | Hooks, UI Automation, event taps, audio sessions, registry |
| Config | `serde` + `serde_json` | Human-readable, atomic writes |
| Logging | `tracing` | Bounded async NDJSON + human-readable log |
| Updates | `ed25519-dalek` + custom feed | Signed manifests, version ordering, rollback |
| Mobile bridge | C ABI + generated bindings | Keeps `draft-core` free of UI and OS dependencies |

## 9. Performance contract

Speed is a release criterion. [`PERFORMANCE.md`](PERFORMANCE.md) owns the
reference machine, the Draft v3.5 baseline, the engine bake-off, and the
benchmark protocol.

- The default final model stays `whisper-large-v3-turbo`; never downgrade it to
  hit a budget.
- Final-text latency must match or beat v3.5 p50 and p95 on the same clips;
  target 20% faster.
- First streamed words < 300 ms after speech onset; the preview model never
  delays the final pass.
- Shortcut press to capture < 30 ms p95 warm; paste adds < 80 ms p95.
- Idle CPU < 0.5%; working set < 300 MB; VRAM < 6.5 GB.
- Installer size without models < 60 MB.
- No dropped audio while the UI is busy or a model is loading.

The v3.5 baseline and the engine choice are measured, not assumed. Until the
baseline is recorded, the absolute ceilings in `PERFORMANCE.md` are upper
bounds only; if v3.5 is faster, v3.5 wins.

## 10. Data and privacy

- Runtime data lives in the platform application-data directory:
  `%LOCALAPPDATA%\Draft4\settings.json` on Windows,
  `~/Library/Application Support/Draft4/settings.json` on macOS, and
  `~/.local/share/Draft4/settings.json` on Linux. Model files live in
  `%LOCALAPPDATA%\Draft4\models` on Windows and the equivalent platform data
  directory; they are re-downloadable.
- Draft v3.5 owns `%LOCALAPPDATA%\Draft`. The rewrite never reads, migrates, or
  overwrites that folder, so the shipping app's `config.json`, `history.json`,
  and logs keep working unchanged while both are installed.
- Model installs accept only the pinned byte length and SHA-256. A download
  writes `<name>.part`, resumes from that prefix, verifies the complete file,
  syncs it, and only then renames it into place; a mismatch is deleted.
- Audio is never persisted by default.
- History stores text and bounded metadata only: no audio, no window titles, no
  full URLs. Defaults are a 30-day window, 2,000 entries, and an 8 MB file; the
  tightest limit wins. Turning retention off accepts no new entries and keeps
  existing ones. A malformed file is quarantined, never silently overwritten.
- A finished transcript is recorded on the app thread after transcription,
  whether or not the paste succeeded. A history-write failure is logged and
  never blocks dictation.
- History timestamps are Unix milliseconds; `draftctl history` prints them as UTC
  so the display never claims a local timezone it cannot know.
- Diagnostic logs may contain transcript text; they stay local and are rotated.
- Network requests are limited to model downloads and update checks.

## 11. Milestones

Status (2026-10-21): M0 is complete on Windows, and the M1 latency/engine slice
is complete while WER and the repeatable paste run remain. M2 has the HUD and
the pinned single-model installer; the Parakeet preview catalog and streaming
remain. M3 has settings, microphone selection, and the bounded history store,
which the app records after each finished transcript; the pane is not wired yet.
M4-M7 are not started.

### M0 - Scaffold and proof

- Repo, license, CI, formatting, Clippy.
- Rust toolchain and Windows build prerequisites documented.
- A GPUI window opens; tray icon appears; global hook logs a chord.
- **Done when:** `cargo run -p draft` shows the window and the hook toggles a log
  line on a real Windows machine. macOS and Linux are proven in M6.

### M1 - Dictation MVP

- Hold Right Ctrl, record, transcribe with the selected model, paste at focus.
- Escape cancels; short taps discard; clipboard is restored.
- **Done when:** a 5-second utterance pastes correctly into Notepad, VS Code,
  and a browser, three times in a row, with the previous clipboard intact.

### M2 - Models and streaming

- Model catalog with a pinned Whisper large-v3-turbo entry; resumable download
  with progress, SHA-256 verification, atomic install, status, and deletion.
- Parakeet stream preview plus Whisper final pass.
- HUD shows waveform, partial text, and processing state.
- **Done when:** text appears while speaking and is replaced by the final
  transcript without flicker or double paste.

### M3 - Settings, history, corrections

- Settings for shortcut, microphone, model, language, paste, sounds, login.
- History with retention, search, copy, delete, clear.
- Corrections and hotwords applied in the pipeline.
- **Done when:** a restart preserves every setting and history entry; a taught
  correction wins on the next dictation.

### M4 - Distribution

- Windows per-user installer (Inno Setup or NSIS) with Start Menu, launch at
  login, and uninstall that keeps user data.
- Signed update feed, version ordering, checksum, rollback.
- macOS and Linux packaging follow their platform rules in M6.
- **Done when:** install → update → uninstall works on a clean Windows VM.

### M5 - Modes and commands

- Foreground-app detection and per-app Modes.
- Transformations.
- More voice commands: send, select all, press enter.
- **Done when:** a mode for VS Code applies only in VS Code and a global mode
  applies everywhere else.

### M6 - Desktop everywhere

- macOS shell (Metal) and Linux shell (Vulkan) over the same core.
- Per-platform permissions, hotkeys, audio devices, paste, tray/menu bar, and
  installers.
- **Done when:** the same settings and pipeline run on all three desktop
  platforms and a dictation started on one platform has the same behavior on the
  others.

### M7 - Mobile

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
| Scope creep | M0-M2 ship a complete dictation loop before any mode or meeting work |

## 13. Open decisions

- Final icon and visual identity (the product name is Draft).
- Whether the first release is x64-only or x64 + ARM64 together.
- Whether iOS or Android ships first in M7.
- Update feed host and signing key custody.
- Whether two-pass streaming is on by default or an "Accuracy" setting.
