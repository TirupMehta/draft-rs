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
- The model loads once on Vulkan and stays warm; release p95 is 2.9-4.1x faster
  than Draft v3.5 on the benchmark clips.
- Tray app with Show, Hide, Paste last, and Quit. Closing the window hides it.
- Click-through recording HUD with listening and transcribing states.
- Preferences persist the shortcut, microphone, and model path to
  `%LOCALAPPDATA%\Draft\settings.json`; choices survive restart.
- Microphone picker with a four-second level check and automatic fallback to the
  system default when a saved device disappears.
- `draftctl` for transcription, microphone capture, and benchmark runs.

Not yet: streaming partial text, history, model downloads, installer and signed
updates, macOS/Linux shells, and mobile. Real-speech WER parity with v3.5 is the
open release gate; see [docs/PERFORMANCE.md](docs/PERFORMANCE.md).

## Why Draft

- **Local.** No cloud, no account, no upload.
- **Fast.** The same `large-v3-turbo` model as Draft v3, kept warm on the GPU,
  with no Python in the path. Benchmarked against v3.5 every release.
- **Ready to stream.** The capture and inference split keeps preview work off
  the final path; the current build pastes the final transcript only.
- **Accurate.** Whisper large-v3-turbo for the final transcript.
- **Polished.** Native GPU-rendered UI, click-through HUD, tray app.
- **Small.** No Python, no PyTorch; models download on demand.
- **Everywhere.** Windows, macOS, and Linux desktop first; mobile shells over
  the same core later.
- **Yours.** MIT licensed, inspectable, no telemetry.

## Repository layout

```text
crates/draft-core/     platform-neutral text, settings, models, engine, history
crates/draft-cli/      development CLI
apps/desktop/          GPUI shell for Windows, macOS, and Linux
apps/mobile/           native iOS and Android shells over the core (later)
docs/                  product spec and roadmap
scripts/               toolchain and release scripts
```

`draft-core` owns behavior. Platform crates own windows, microphones, hotkeys,
clipboards, trays, updaters, and permissions. No OS-specific type crosses into
the core.

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

The app looks for `%LOCALAPPDATA%\Draft\models\whisper-large-v3-turbo-Q8_0.gguf`.
Pass `--model PATH` or set `DRAFT_MODEL_PATH` for a local GGUF. The development
CLI is `cargo run -p draft -- --help` (binary `draftctl`). Run
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
and `cargo test --workspace` before committing.

## License

MIT — see [LICENSE](LICENSE).
