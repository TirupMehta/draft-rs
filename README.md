# Draft

**Speak. It appears.** Local-first voice dictation for every device, built in
Rust.

Draft is a native, GPU-accelerated dictation app: hold a key, speak, release,
and accurate text appears at your cursor. Speech never leaves the machine.
Streaming previews appear while you talk; the final pass uses Whisper
large-v3-turbo for accuracy.

> **Status:** pre-alpha. The Rust core and desktop shell are being built. The
> shipping Windows app is still [Draft v3](../Draft).

## Why Draft

- **Local.** No cloud, no account, no upload.
- **Fast.** The same `large-v3-turbo` model as Draft v3, kept warm on the GPU,
  with no Python in the path. Benchmarked against v3.5 every release.
- **Streaming.** Words appear while you speak.
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

## License

MIT — see [LICENSE](LICENSE).
