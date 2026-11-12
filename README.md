# Draft

Draft is a private, on-device dictation app for Windows. You hold a key, speak,
and release, and the transcript is pasted at your cursor. Speech recognition
runs locally with [Whisper](https://github.com/openai/whisper), on the GPU when
one is available, so your audio is never uploaded and no account is needed.

![Draft, ready to dictate](docs/screenshots/dictate.png)

Draft is free software under the MIT license. It is an independent project and
is not affiliated with OpenAI.

## Features

*   Hold-to-talk dictation into any application that accepts pasted text.
*   Local transcription with Whisper. Audio is captured only while the key is
    held, then discarded.
*   Seven model sizes, from 85 MB to 1.6 GB. Draft measures your PC and
    recommends one.
*   Paragraph breaks at real pauses, spoken commands, and optional lists.
    Formatting never changes a word.
*   A local History tab, a tray icon, and a recording indicator that never takes
    focus.
*   No telemetry, analytics, crash reporting, or update check.

## Installation

Draft requires Windows 10 or 11 (x64) on a CPU with AVX2, which includes any PC
made since 2013. A Vulkan GPU gives the fast path. Without one, Draft runs on
the CPU and is slower.

Download an installer from the
[latest release](https://github.com/TirupMehta/draft-rs/releases/latest) and run
it. The installer is per-user and does not need administrator rights.

| Installer                                 | Size    | Model                                      |
| ----------------------------------------- | ------- | ------------------------------------------ |
| `Draft-Setup-<version>.exe`               | 11 MB   | Downloaded once on first launch            |
| `Draft-Setup-<version>-offline-small.exe` | 257 MB  | Whisper Small, included                    |
| `Draft-Setup-<version>-offline.exe`       | 1.43 GB | Whisper Large v3 Turbo, included           |

Use the standard installer unless the target PC has no internet connection. On
first launch it picks a model for your hardware, downloads it, and verifies its
checksum. Of the offline installers, the small one suits older PCs and machines
without a GPU, and the full one suits PCs with a capable GPU.

The installer is not code-signed yet, so Windows SmartScreen may show a warning
on the first run. Choose **More info**, then **Run anyway**. To verify a
download, compare its hash with the `.sha256` file on the release page:

```powershell
Get-FileHash .\Draft-Setup-<version>.exe -Algorithm SHA256
```

To uninstall, use **Uninstall Draft** in the Start menu. Your settings, history,
and model in `%LOCALAPPDATA%\Draft4` are kept so that a reinstall does not
download the model again. Delete that folder to remove them.

An early Apple Silicon preview is available on the
[macOS prerelease](https://github.com/TirupMehta/draft-rs/releases/tag/v0.2.0-macos.1).
It is not notarized and has not been run on a physical Mac. See
[docs/MACOS.md](docs/MACOS.md). Linux is not supported.

## Usage

Hold **Right Ctrl**, speak, and release. The transcript is pasted at the cursor.
Press **Esc** while recording to cancel. In Settings you can change the key to
Right Alt, F8, F9, Pause, or Scroll Lock.

While you hold the key, a small indicator shows the input level and the length
of the take. It does not take focus from the application you are typing in.

**Formatting.** Draft inserts a paragraph break at a real pause, measured
against your own speaking rhythm, which it learns on your PC. It never rewrites,
adds, or reorders a word.

**Spoken commands.** Say `new paragraph`, `new line`, or `scratch that` as its
own phrase, with a pause on each side. The same words inside a sentence are left
alone. Lists are off by default; when enabled, `make a list` and
`make a numbered list` put one item on each line.

**Live preview.** Shows words as you speak. It is off by default because each
preview is another GPU pass. The final transcript always comes from the whole
clip.

**History.** The History tab lists your newest 50 dictations and lets you copy
or delete them. It stores text and duration only, never audio, and removes
entries older than 30 days.

**Tray.** The tray menu has Show Draft, Hide Draft, Paste last dictation, and
Quit Draft. Draft starts in the tray at sign-in unless you turn that off.

## Models

Draft marks one model as recommended for your PC and downloads it once from a
pinned Hugging Face file, verifying its SHA-256. You can change the model at any
time in Settings, Recognition.

| Step              | Model                          | Size   | Suits                            |
| ----------------- | ------------------------------ | ------ | -------------------------------- |
| Most accurate     | Whisper Large v3 Turbo, F16    | 1.6 GB | A GPU with about 5 GB of memory  |
| Accurate          | Whisper Large v3 Turbo, Q8_0   | 886 MB | GPUs from 3 GB                   |
| Accurate, compact | Whisper Large v3 Turbo, Q5_K_M | 620 MB | 2 GB GPUs and tight memory       |
| Balanced          | Whisper Small, Q8_0            | 270 MB | Most PCs, with or without a GPU  |
| Balanced, English | Whisper Small English, Q8_0    | 270 MB | As above, English only           |
| Light             | Whisper Base, Q8_0             | 85 MB  | Older or slower PCs              |
| Light, English    | Whisper Base English, Q8_0     | 85 MB  | The smallest English model       |

## Performance

The figures below were measured on one reference machine: an Intel Core i7-13620H
with an NVIDIA GeForce RTX 4060 Laptop GPU (8 GB), Windows 11, the Vulkan
backend, the F16 model, and a warm model. Each is the p95 of five runs. They
cover decoding only, before the paste.

| Audio | Draft    | Previous app (v3.5) | Speedup |
| ----- | -------- | ------------------- | ------- |
| 3 s   | 239 ms   | 996 ms              | 4.17x   |
| 5 s   | 234 ms   | 929 ms              | 3.97x   |
| 15 s  | 375 ms   | 1,139 ms            | 3.04x   |
| 30 s  | 543 ms   | 1,601 ms            | 2.95x   |
| 60 s  | 1,186 ms | 3,036 ms            | 2.56x   |

A minute of speech is transcribed in about 1.2 seconds, roughly 50 times faster
than real time. Short clips carry a fixed cost of about 0.2 seconds, so a
3-second clip is not proportionally faster. These numbers describe one laptop
and are not a guarantee for other hardware. Without a GPU, Draft runs on the CPU
and takes a few seconds per clip. The method and the raw reports are in
[docs/PERFORMANCE.md](docs/PERFORMANCE.md).

## Privacy

*   Audio is captured only while the dictation key is held. It is transcribed on
    your PC and discarded.
*   History contains text and duration only. Settings, history, logs, a learned
    pause profile (timing numbers, no words), and the model are stored in
    `%LOCALAPPDATA%\Draft4`.
*   The only network request is the speech model. The standard installer
    downloads it once; the offline installers include one.
*   Draft sends no telemetry and does not check for updates. Updates are manual.

## Building from source

Building requires Rust (stable), the Microsoft C++ Build Tools, CMake, and the
Vulkan SDK.

```shell
git clone https://github.com/TirupMehta/draft-rs.git
cd draft-rs
cargo run -p draft-desktop --release
```

Before sending a change, run the same checks as CI:

```shell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

To build the installer, install Inno Setup 6, build the release binary, copy it
to `dist\draft.exe`, and run `scripts\build-installer.ps1`. Pass `-IncludeModel`
for an offline installer, and `-ModelPath` and `-Suffix` to choose its model and
file name. The `draftctl` tool in the same workspace manages models, searches
history, and runs the benchmarks.

The code is a Cargo workspace. `draft-core` holds the platform-independent
logic, `draft-audio`, `draft-hotkey`, and `draft-paste` handle the microphone,
the key, and the clipboard, and `apps/desktop` is the user interface. See
[AGENTS.md](AGENTS.md) for a full map.

## Project status

Draft is an early release.

*   The installer is not code-signed.
*   Accuracy compared with the previous app (v3.5) on real speech is still being
    measured, and Draft makes no accuracy claim until it is.
*   There is no in-app updater.

## Documentation

*   [docs/SPEC.md](docs/SPEC.md): product intent and scope.
*   [docs/PERFORMANCE.md](docs/PERFORMANCE.md): benchmark method and results.
*   [docs/ROADMAP.md](docs/ROADMAP.md): open work and release gates.
*   [docs/MACOS.md](docs/MACOS.md): what is and is not verified on macOS.
*   [docs/CODE_SIGNING.md](docs/CODE_SIGNING.md): plan for signing the installer.
*   [AGENTS.md](AGENTS.md): working notes for anyone changing the code.

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) before
opening a change. It covers the setup, the design rules, and the checks that
must pass.

## License

Draft is released under the [MIT License](LICENSE). The bundled typefaces are
under the SIL Open Font License 1.1. The speech model is distributed with Draft
and is subject to its own terms. See [NOTICE](NOTICE).

## Acknowledgments

Draft is designed and built by [Tirup Mehta](https://github.com/TirupMehta). It
is built on [GPUI](https://github.com/zed-industries/zed) by Zed Industries,
transcribe.cpp, Whisper by OpenAI in a GGUF build by handy-computer, and the
Instrument Sans and Instrument Serif typefaces. [CREDITS.md](CREDITS.md) has the
full list, and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) lists every Rust
dependency.
