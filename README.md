# Draft

**Speak. It appears.** Local-first voice dictation for every device, built in
Rust.

Draft is a native, GPU-accelerated dictation app: hold a key, speak, release,
and accurate text appears at your cursor. Speech never leaves the machine. The
final pass uses Whisper large-v3-turbo, kept warm on the GPU; the capture and
inference split is built for streaming previews without slowing the final
transcript.

> **Status:** pre-alpha. The Windows x64 shell runs the hold-to-talk loop today.
> macOS, Linux, and mobile are later milestones, and the shipping Windows app is
> still [Draft v3](../Draft).

## What works today

- Hold Right Ctrl (or Right Alt, F8, F9, Pause, or Scroll Lock), speak, release,
  and paste locally at the cursor.
- Opt-in live partial text while you hold the key. Settings > Live preview lets
  the warm model re-read voiced audio and show words before you release. It is
  off by default because each preview is a full GPU pass that can make laptop
  fans louder. The final transcript still uses the complete clip, and previews
  never paste.
- The model loads once on Vulkan and stays warm; release p95 is 2.9-4.1x faster
  than Draft v3.5 on the benchmark clips.
- Tray app with Show, Hide, Paste last, and Quit. Closing the window hides it.
  Draft starts in the tray when you sign in; turn that off under Settings >
  Startup.
- One seamless window: Draft draws its own title bar, so the chrome and the
  body are a single surface with one wordmark and one hairline.
- Settings are grouped by intent — Dictation, Audio, Recognition, Appearance,
  Privacy, Advanced — and every control eases into place instead of jumping.
- Light and dark themes, both designed; the system setting is the default.
- Click-through recording HUD with a live waveform, optional partial text, and
  listening and transcribing states.
- Preferences persist the shortcut, microphone, and model path to
  `%LOCALAPPDATA%\Draft4\settings.json`; choices survive restart.
- Microphone picker with a four-second level check and automatic fallback to the
  system default when a saved device disappears.
- `draftctl model install` downloads the shipping model once, verifies its exact
  SHA-256, and installs it atomically; interrupted downloads resume.
- Retained history at the core: text plus duration, with a retention window, an
  entry cap, a byte cap, search, delete, and clear (`draftctl history`). The app
  records each finished transcript; the History pane is not built yet.
- A per-user Windows installer with a standard wizard, Start Menu and optional
  desktop shortcuts, tray-first sign-in startup, and a clean uninstall. No
  administrator prompt and no console window. It is not code-signed yet.
- `draftctl` for model management, transcription, microphone capture, and
  benchmark runs.

Not yet: the native streaming preview model, the history pane, the multi-model
catalog, signed installers and updates, macOS/Linux shells, and mobile.
Real-speech WER parity with v3.5 is the open release gate; see
[docs/PERFORMANCE.md](docs/PERFORMANCE.md).

Draft v3.5 keeps its data in `%LOCALAPPDATA%\Draft`. This rewrite uses
`%LOCALAPPDATA%\Draft4` and never reads, migrates, or overwrites the shipping
app's settings, history, or logs.

## Why Draft

- **Local.** No cloud, no account, no upload.
- **Fast.** The same `large-v3-turbo` model as Draft v3, kept warm on the GPU,
  with no Python in the path. Benchmarked against v3.5 every release.
- **Quiet by default.** One final GPU pass per dictation; live preview is an
  explicit opt-in. The capture and inference split keeps preview work off the
  final path, and the current build pastes the final transcript only.
- **Accurate.** Whisper large-v3-turbo for the final transcript.
- **Polished.** Native GPU-rendered UI, click-through HUD, tray app.
- **Small.** No Python, no PyTorch; models download on demand.
- **Everywhere.** Windows, macOS, and Linux desktop first; mobile shells over
  the same core later.
- **Yours.** MIT licensed, inspectable, no telemetry.

## Repository layout

```text
crates/draft-core/     platform-neutral text, settings, models, history
crates/draft-engine/   GGUF inference through transcribe.cpp
crates/draft-audio/    microphone capture and resampling
crates/draft-hotkey/  the global dictation key
crates/draft-paste/   clipboard insertion
crates/draft-cli/      development CLI (draftctl)
apps/desktop/          GPUI shell for Windows, macOS, and Linux
docs/                  product spec, roadmap, performance, handoff
scripts/               Windows toolchain and release helpers
tools/inspect/         render inspection (text reports of the real window)
```

`draft-core` owns behavior. Platform crates own windows, microphones, hotkeys,
clipboards, trays, updaters, and permissions. No OS-specific type crosses into
the core.

**Nothing in the shipped product needs PowerShell.** Draft is a single
self-contained `draft.exe`; the installer is a single `Draft-Setup-<version>.exe`.
The `.ps1` files are developer-side only: `scripts/` sets up a Windows
toolchain and builds the installer, and `tools/inspect/` prints the rendered
window as text so the interface can be checked without a screenshot. See
[AGENTS.md](AGENTS.md) for the working rules.

## Stack

| Layer | Choice |
| --- | --- |
| Desktop UI | GPUI (native, GPU-rendered) |
| Mobile UI | SwiftUI and Jetpack Compose over the core (later) |
| Audio | cpal (WASAPI, CoreAudio, ALSA/PipeWire) |
| Inference | transcribe.cpp (GGUF Whisper large-v3-turbo; Vulkan, Metal, CUDA) |
| Clipboard | arboard + native synthetic input |
| Platform | windows-sys, objc2, X11/Wayland |

See [docs/SPEC.md](docs/SPEC.md) for the product contract and architecture,
[docs/PERFORMANCE.md](docs/PERFORMANCE.md) for the speed baseline and release
gate, and [docs/ROADMAP.md](docs/ROADMAP.md) for the build order.

## Build

Requirements: Rust stable, a native C/C++ toolchain, CMake, and a GPU SDK for the
platform (Vulkan on Windows/Linux, Metal on macOS).

Windows:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\setup-windows.ps1
cargo run -p draft-desktop
```

The app looks for
`%LOCALAPPDATA%\Draft4\models\whisper-large-v3-turbo-F16.gguf`. Run
`cargo run -p draft -- model install` to download and verify it, or pass
`--model PATH` / set `DRAFT_MODEL_PATH` for a local GGUF. `draftctl model status`
and `model verify` report what is installed. The development CLI is
`cargo run -p draft -- --help` (binary `draftctl`). Run
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
and `cargo test --workspace` before committing.

### Windows installer

Inno Setup 6 is the only extra tool (`winget install JRSoftware.InnoSetup`).
Build the release binary first, then the wizard:

```powershell
cargo build --release -p draft-desktop
.\scripts\build-installer.ps1
```

That writes `dist\Draft-Setup-<version>.exe`, a per-user install under
`%LOCALAPPDATA%\Programs\Draft` with no administrator prompt. Add
`-IncludeModel` to embed the installed F16 GGUF for a fully offline install; the
installer then skips the copy when `%LOCALAPPDATA%\Draft4\models` already holds
it. Uninstall removes the app and shortcuts but keeps settings, history, logs,
and models.

## License

MIT — see [LICENSE](LICENSE).
