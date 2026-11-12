# Draft

Private, on-device dictation for Windows.

[![Latest release](https://img.shields.io/github/v/release/TirupMehta/draft-rs?style=flat-square&color=a3610c&labelColor=1b1915)](https://github.com/TirupMehta/draft-rs/releases/latest)
![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011%20x64-1b1915?style=flat-square)
[![License](https://img.shields.io/badge/license-MIT-1b1915?style=flat-square)](LICENSE)
![Telemetry](https://img.shields.io/badge/telemetry-none-1b1915?style=flat-square)

Draft turns speech into text at your cursor. You hold a key, speak, and release;
the transcript is pasted into whatever you were typing in. Transcription runs
on your own machine with Whisper, on the GPU when one is available, so your
audio is never uploaded and there is no account to create.

<p align="center">
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/dictate-dark.png">
  <img src="docs/screenshots/dictate.png" alt="Draft, ready to dictate" width="680">
</picture>
</p>

## Contents

- [Overview](#overview)
- [Quick start](#quick-start)
- [Usage](#usage)
- [Screenshots](#screenshots)
- [Performance](#performance)
- [Privacy](#privacy)
- [Status and limitations](#status-and-limitations)
- [Documentation](#documentation)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Overview

| | |
| --- | --- |
| **Local** | Whisper large-v3-turbo runs on your PC. Audio is captured only while the key is held, then discarded. |
| **Fast** | About 50× faster than you speak on a GPU: a minute of speech in 1.19 s (p95). See [Performance](#performance). |
| **Configurable** | A seven-step model ladder from 85 MB to 1.6 GB. Draft measures your PC and recommends a step. |
| **Readable** | Paragraph breaks at real pauses, spoken commands, optional lists. Formatting never changes a word. |
| **Native** | A Rust and GPUI app with a tray icon, a click-through recording HUD, and a per-user installer that needs no administrator rights. |
| **Open** | MIT licensed. No telemetry, analytics, crash reporting, or update ping. |

## Quick start

### Requirements

| | |
| --- | --- |
| **OS** | Windows 10 or 11, x64, with AVX2 (any PC from 2013 on) |
| **Graphics** | Any Vulkan GPU for the fast path. Without one, Draft runs on the CPU and is slower. |
| **Disk** | The installer, plus the speech model (85 MB to 1.6 GB) |

### Install

Download an installer from the [latest release](https://github.com/TirupMehta/draft-rs/releases/latest):

| Installer | Size | Model | Use it when |
| --- | --- | --- | --- |
| `Draft-Setup-0.5.1.exe` **(recommended)** | 11 MB | Downloaded on first launch, chosen for your PC | You are online. This is the right choice for almost everyone. |
| `Draft-Setup-0.5.1-offline-small.exe` | 257 MB | Whisper Small, included | You are offline and have an older PC or no GPU. |
| `Draft-Setup-0.5.1-offline.exe` | 1.43 GB | Whisper large-v3-turbo, included | You are offline and have a capable GPU. |

1. Run the installer. It installs for the current user under
   `%LOCALAPPDATA%\Programs\Draft`; no administrator prompt is needed.
2. On first launch, the standard installer measures your PC, picks a model, and
   downloads it once with a progress bar. The offline installers already
   include theirs.
3. Hold **Right Ctrl**, speak, and release. The text appears at your cursor.

The installer is **not code-signed yet**, so Windows SmartScreen may show a
warning on first run: choose **More info**, then **Run anyway**. Each installer
has a `.sha256` file on the release page; verify the download first:

```powershell
Get-FileHash .\Draft-Setup-0.5.1.exe -Algorithm SHA256
```

### Uninstall

Start menu → Draft → **Uninstall Draft**. This removes the app, its shortcuts,
and its license files. Your settings, history, and speech model in
`%LOCALAPPDATA%\Draft4` are kept so a reinstall does not download the model
again. Delete that folder by hand for a clean slate.

### macOS (Apple Silicon preview)

Download `Draft-0.2.0-macos-arm64.dmg` from the
[macOS prerelease](https://github.com/TirupMehta/draft-rs/releases/tag/v0.2.0-macos.1).
It is ad-hoc signed and not notarized, so macOS blocks the first launch; the
release notes list the exact steps. It is a preview: it builds and links on
Apple hardware, but no one has dictated on a physical Mac yet.

## Usage

Hold **Right Ctrl** (or Right Alt, F8, F9, Pause, or Scroll Lock), speak, and
release. The transcript is pasted at the cursor. Press **Esc** while recording
to cancel the take.

| Feature | Behaviour |
| --- | --- |
| Live preview | Off by default (Settings → Dictation). The final transcript always comes from the complete clip. |
| Formatting | A paragraph break at a real pause, measured against your own rhythm. Nothing is rewritten, added, or reordered. |
| Spoken commands | Say `new paragraph`, `new line`, or `scratch that` as its own phrase, with a pause on each side. Ordinary speech containing those words is left alone. |
| History | The newest dictations with copy and delete. Text and duration only, never audio, window titles, or full URLs. `draftctl history` searches all of it. |
| Models | Seven steps from an 85 MB English model to the 1.6 GB large model, downloaded once with a resumable, SHA-256-verified install. |
| Microphone | A picker with a level check, and automatic fallback to the system default when a saved device disappears. |
| Tray | Show, Hide, Paste last, and Quit. Draft starts in the tray at sign-in by default; you can turn that off. |
| Theme | Light by default, with a designed dark theme. Every view shares one fixed 680×500 surface. |

## Screenshots

| History | Recognition |
| --- | --- |
| <picture>   <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/history-dark.png">   <img src="docs/screenshots/history.png" alt="History tab"> </picture> | <picture>   <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/recognition-dark.png">   <img src="docs/screenshots/recognition.png" alt="Recognition settings"> </picture> |
| Your newest dictations, text and duration only. Copy or delete any of them. | Pick a model, or let Draft recommend one for your PC. |

| Formatting | Privacy |
| --- | --- |
| <picture>   <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/formatting-dark.png">   <img src="docs/screenshots/formatting.png" alt="Formatting settings"> </picture> | <picture>   <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/privacy-dark.png">   <img src="docs/screenshots/privacy.png" alt="Privacy settings"> </picture> |
| Paragraphs at real pauses, spoken commands, optional lists. | What happens to one dictation, and where your data lives. |

The recording HUD is one small pill that floats over whatever you are typing
into and never takes focus: a dot, a live meter, and the length of the take.
It widens only when live preview is on, and shows "Transcribing" while the
model works. Light and dark follow your theme:

<p align="center">
  <img src="docs/screenshots/hud-states.png" alt="The recording HUD in its four states, in light and dark: just pressed, speaking, speaking with live preview, and transcribing" width="560">
</p>

<p align="center"><sub>Illustration of the four HUD states. The words in the live preview row are an example.</sub></p>

## Performance

Measured on the reference machine: Intel Core i7-13620H, NVIDIA GeForce RTX
4060 Laptop 8 GB, Windows 11, Vulkan, F16 model, warm model, five measured
runs per clip. Decode only, before clipboard paste.

| Audio | Draft p95 | Draft v3.5 p95 | Speedup |
| --- | --- | --- | --- |
| 3 s | **239 ms** | 996 ms | 4.17× |
| 5 s | **234 ms** | 929 ms | 3.97× |
| 15 s | **375 ms** | 1,139 ms | 3.04× |
| 30 s | **543 ms** | 1,601 ms | 2.95× |
| 60 s | **1,186 ms** | 3,036 ms | 2.56× |
| JFK, 11 s | **264 ms** | - | - |

Short clips carry a fixed cost of roughly 0.2 s (the encoder always processes a
30-second window), so a 3-second clip is not proportionally faster than a
5-second one. These numbers are a baseline, not a promise for every GPU.

**Without a GPU**, the same F16 model runs on the CPU. The app asks for three
quarters of the logical CPUs rather than all of them, which measured 26-40%
faster than the default and leaves the interface responsive, and it skips the
GPU-only warm-up so the first dictation is available in a few seconds. CPU
dictation takes a few seconds per clip, not the sub-second GPU figure. The
method, the measured CPU table, and the raw reports are in
[docs/PERFORMANCE.md](docs/PERFORMANCE.md).

## Privacy

Draft is built to be private without asking you to trust a policy.

- Audio is captured **only while the dictation key is held**.
- Speech is transcribed on this device and discarded. Draft does not keep the
  recording.
- History contains **text and duration only**.
- Settings, history, logs, and the model live under
  `%LOCALAPPDATA%\Draft4`.
- The one network exception is the speech model. The standard installer is
  small and downloads the model you choose once, on first launch, from the
  pinned Hugging Face source (85 MB to 1.6 GB), verified against a pinned
  SHA-256. The offline installers bundle a model instead. After the
  model is on your PC, dictation needs no network.
- There is no telemetry, analytics, crash reporting, or update ping.
- The GitHub and X actions on the Credits page open your browser only when you
  choose them.

## Status and limitations

Draft 0.5.1 is an early Windows release.

- Windows x64 is the supported release. An Apple Silicon macOS preview is
  available as an ad-hoc-signed DMG: it builds and links the Metal engine on the
  `macos-14` runner and carries the same event tap, `Cmd+V` paste, AppKit window
  behaviour, permission onboarding, and `Draft.app` bundle. It is not notarized
  and has not been run on a physical Mac, so treat it as a preview, not a
  supported release. [docs/MACOS.md](docs/MACOS.md) lists what is and is not
  verified. Linux is not planned.
- The installer is unsigned.
- Real-speech WER parity with Draft v3.5 is still being measured. It is the
  next release gate, and no accuracy claim is made ahead of it.
- Updates are manual; there is no in-app updater.

## Documentation

| Document | Contents |
| --- | --- |
| [docs/SPEC.md](docs/SPEC.md) | Product intent and scope |
| [docs/PERFORMANCE.md](docs/PERFORMANCE.md) | Benchmark method, CPU and GPU tables, raw reports |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Open work and release gates |
| [docs/MACOS.md](docs/MACOS.md) | macOS status and what is verified |
| [docs/CODE_SIGNING.md](docs/CODE_SIGNING.md) | Signing plan for the Windows installer |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Setup, design rules, required checks |
| [CREDITS.md](CREDITS.md), [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) | Attribution and dependency licenses |

## Development

<details>
<summary><b>Build from source</b></summary>

Requirements: Rust stable, Microsoft C++ Build Tools, CMake, and the Vulkan SDK.
Install those, then:

```text
git clone https://github.com/TirupMehta/draft-rs.git
cd draft-rs
cargo run -p draft-desktop
```

The app looks for the F16 model at
`%LOCALAPPDATA%\Draft4\models\whisper-large-v3-turbo-F16.gguf`. To download and
verify it from the command line:

```text
cargo run -p draft -- model install
cargo run -p draft -- model status
```

`draftctl` also exposes `model list`, `model verify`, `model remove`,
`transcribe`, `bench`, `mic`, `listen`, and `history`.

Run the same gates as CI before sending a change:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

### Installer

Inno Setup 6 is the only extra tool. The release installer builder is
`scripts/build-installer.ps1`:

```text
cargo build --workspace --release
.\scripts\build-installer.ps1
```

That writes `dist\Draft-Setup-<version>.exe`. Add `-IncludeModel` to embed the
installed GGUF for an offline install.

Uninstall from **Start → Draft → Uninstall Draft**, or from Settings → Apps.
It removes the app, both shortcuts, and the installed license texts. Settings,
history, logs, and the model under `%LOCALAPPDATA%\Draft4` are intentionally
kept; delete that folder yourself for a full reset.

</details>

<details>
<summary><b>Architecture</b></summary>

```text
crates/draft-core      settings, history, text cleanup, model catalog, paths
crates/draft-engine    transcribe.cpp GGUF inference (Vulkan / CPU)
crates/draft-audio     cpal capture, 16 kHz resample, live level
crates/draft-hotkey    hold-to-talk key: platform-neutral config, Windows hook
crates/draft-paste     clipboard insertion
crates/draft-cli       the draftctl development binary
apps/desktop           the GPUI shell (the product UI)
assets                 app icon and bundled Instrument Sans / Serif
bench                  fixtures, baseline data, latency measurements
docs                   specification, roadmap, performance, macOS plan
packaging/windows      the Inno Setup installer
scripts                build-installer.ps1, the release installer builder
```

`draft-core` owns behavior that is not platform-specific. Platform crates own
windows, microphones, hotkeys, clipboards, trays, and permissions. No OS-specific
type crosses into the core.

</details>

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a change. It covers the
Windows setup, the design rules, the checks that must pass, and the license.

## License

Draft is released under the [MIT License](LICENSE). The bundled typefaces are
under the SIL Open Font License 1.1. The speech model is distributed with
Draft and is subject to its own terms. See [NOTICE](NOTICE),
[CREDITS.md](CREDITS.md), and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Credits

Draft is designed and built by [Tirup Mehta](https://github.com/TirupMehta).
It is built on **GPUI** by Zed Industries (Apache-2.0), **transcribe.cpp**
(MIT), **Whisper large-v3-turbo** by OpenAI in a GGUF build by handy-computer,
**Instrument Sans** and **Instrument Serif** (OFL 1.1). The full list is in [CREDITS.md](CREDITS.md), and every Rust
dependency is listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

The Credits section is also available in the app, under Settings → Credits,
with links to GitHub and X and a button that opens the installed license files.
