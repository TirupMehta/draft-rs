# Draft

**Speak. It appears.** Local-first voice dictation for Windows, built in Rust.

Draft is a native, GPU-accelerated dictation app: hold a key, speak, release,
and accurate text appears at your cursor. Speech never leaves the machine.
Streaming previews appear while you talk; the final pass uses Whisper
large-v3-turbo for accuracy.

> **Status:** pre-alpha. The Rust shell is being built. The shipping Windows app
> is still [Draft v3](../Draft).

## Why Draft

- **Local.** No cloud, no account, no upload.
- **Streaming.** Words appear while you speak.
- **Accurate.** Whisper large-v3-turbo for the final transcript.
- **Polished.** Native GPU-rendered UI, click-through HUD, tray app.
- **Small.** No Python, no PyTorch; models download on demand.
- **Yours.** MIT licensed, inspectable, no telemetry.

## Stack

| Layer | Choice |
| --- | --- |
| UI | GPUI (native, GPU-rendered) |
| Audio | cpal (WASAPI) |
| Inference | transcribe.cpp (GGUF Parakeet + Whisper) |
| Clipboard | arboard + Win32 SendInput |
| Tray | tray-icon |
| Platform | windows-sys / windows |

See [docs/SPEC.md](docs/SPEC.md) for the product contract and architecture, and
[docs/ROADMAP.md](docs/ROADMAP.md) for the build order.

## Build

Requirements: Windows 10/11, Rust stable, Visual Studio C++ Build Tools, Windows
SDK, CMake, and the Vulkan SDK for GPU inference.

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\setup-windows.ps1
cargo run
```

## License

MIT — see [LICENSE](LICENSE).
