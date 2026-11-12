<p align="center">
  <img src="assets/draft-256.png" alt="Draft" width="96">
</p>

<h1 align="center">Draft</h1>

<p align="center">
  <strong>Private dictation for Windows.</strong><br>
  Hold a key, speak, release. The words appear at your cursor.<br>
  The model runs on your machine, so your voice never leaves it.
</p>

<p align="center">
  Whisper large-v3-turbo · Vulkan · Native Rust · Free · MIT · No account · No telemetry
</p>

<p align="center">
  <a href="https://github.com/TirupMehta/draft-rs/releases/latest"><strong>Download</strong></a> ·
  <a href="#install">Install</a> ·
  <a href="#performance">Performance</a> ·
  <a href="#privacy">Privacy</a> ·
  <a href="https://x.com/TirupMehta">X</a>
</p>

<p align="center">
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/dictate-dark.png">
  <img src="docs/screenshots/dictate.png" alt="Draft, ready to dictate" width="680">
</picture>
</p>

## Why Draft

- **Private by construction.** Audio is captured only while the key is held,
  transcribed on this PC, and discarded. Nothing is uploaded, and there is no
  account, telemetry, or update ping.
- **Fast on a GPU.** Draft transcribes about **50× faster than you speak**: a
  minute of speech takes 1.19 s p95 on the reference machine, 2.56× faster than
  v3.5. Clips of 5 to 15 seconds take under 0.4 s. Without a GPU, dictation
  takes seconds. See [Performance](#performance).
- **Your choice of model.** Whisper large-v3-turbo by default, with a seven-step
  speed and accuracy ladder down to an 85 MB English model. Draft measures your
  PC and recommends one.
- **Readable output.** Paragraph breaks at real pauses, spoken commands, and
  optional lists. Formatting never changes a word.
- **One clear gesture.** Six shortcut choices, a real input meter, and no
  modes to learn.
- **A real Windows app.** A fixed, GPU-rendered window, a click-through
  recording HUD, a tray icon, and a per-user installer with no administrator
  prompt.
- **Honest.** The interface shows the real model state, the real input level,
  and the real latency. Nothing is simulated.

## A look inside

| History | Recognition |
| --- | --- |
| <picture>   <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/history-dark.png">   <img src="docs/screenshots/history.png" alt="History tab"> </picture> | <picture>   <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/recognition-dark.png">   <img src="docs/screenshots/recognition.png" alt="Recognition settings"> </picture> |
| Your newest dictations, text and duration only. Copy or delete any of them. | Pick a model, or let Draft recommend one for your PC. |

| Formatting | Privacy |
| --- | --- |
| <picture>   <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/formatting-dark.png">   <img src="docs/screenshots/formatting.png" alt="Formatting settings"> </picture> | <picture>   <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/privacy-dark.png">   <img src="docs/screenshots/privacy.png" alt="Privacy settings"> </picture> |
| Paragraphs at real pauses, spoken commands, optional lists. | What happens to one dictation, and where your data lives. |

The recording HUD floats over whatever you are typing into and never takes focus:

![Recording HUD](docs/screenshots/hud.png)

## What works today

- Hold **Right Ctrl** (or Right Alt, F8, F9, Pause, or Scroll Lock), speak,
  release, and the transcript is pasted at the cursor. Press **Esc** while
  recording to cancel.
- **Live preview** (Settings → Dictation) is off by default; the final
  transcript always comes from the complete clip.
- **Formatting** turns a finished transcript into readable text: a paragraph
  break at a real pause, measured against your own rhythm. Nothing is
  rewritten, added, or reordered.
- **Spoken commands**: say `new paragraph`, `new line`, or `scratch that` as its
  own phrase, with a pause on each side. Ordinary speech containing those words
  is left alone.
- **History tab**: the newest dictations with copy and delete, and a two-click
  delete all. Text and duration only, never audio, window titles, or full URLs;
  `draftctl history` searches all of it.
- **Model ladder**: seven steps from an 85 MB English model to the full 1.6 GB
  large model, downloaded once with a resumable, SHA-256-verified install.
- The model loads once and stays warm. Vulkan is the shipping path, with a CPU
  fallback when no GPU backend is available.
- Light by default, with a designed dark theme. Dictate, History, and Settings
  share one fixed 680×500 surface, so nothing resizes or jumps.
- A microphone picker with a level check and automatic fallback to the system
  default when a saved device disappears.
- Tray actions for Show, Hide, Paste last, and Quit. Draft starts in the tray at
  sign-in by default; you can turn that off.
- A per-user installer to `%LOCALAPPDATA%\Programs\Draft` with a Start-menu
  **Uninstall Draft**. Uninstall keeps your settings, history, and model in
  `%LOCALAPPDATA%\Draft4` so a reinstall does not start over.

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
  SHA-256. The `-offline` installer bundles the large model instead. After the
  model is on your PC, dictation needs no network.
- There is no telemetry, analytics, crash reporting, or update ping.
- The GitHub and X actions on the Credits page open your browser only when you
  choose them.

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

## Install

**Windows 10/11, x64 with AVX2.** Draft's CPU fallback is built for an AVX2
baseline (Haswell, 2013, or newer), which every Windows 11 machine meets.

1. Download `Draft-Setup-0.5.0.exe` from
   [Releases](https://github.com/TirupMehta/draft-rs/releases/latest). Choose
   `Draft-Setup-0.5.0-offline.exe` (1.43 GB) only if you want no first-run
   download.
2. Run the installer. It installs for the current user under
   `%LOCALAPPDATA%\Programs\Draft`; no administrator prompt is needed.
3. On first launch Draft picks a model for your PC and downloads it once, with
   a progress bar. The offline installer already includes the large model.
4. Hold **Right Ctrl**, speak, and release. The text appears at your cursor.

**To remove it:** Start menu → Draft → **Uninstall Draft**. That one entry
removes the app, its shortcuts, and its license files. Your settings, history,
and the speech model in `%LOCALAPPDATA%\Draft4` are kept, so reinstalling does
not download the model again. Delete that folder by hand if you want a clean
slate.

The installer is **not code-signed yet**, so Windows SmartScreen may show a
warning on first run. You can verify the download against the release notes, or
build from source.

**Apple Silicon macOS, preview.** Download `Draft-0.2.0-macos-arm64.dmg` from
the [macOS prerelease](https://github.com/TirupMehta/draft-rs/releases/tag/v0.2.0-macos.1).
It is ad-hoc signed, not notarized, so macOS blocks the first launch; the
release notes list the exact steps. The DMG carries the F16 model. The app is a
preview: it builds and links on Apple hardware, but no one has dictated on a
physical Mac yet.

## Build from source

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

## Architecture

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

## Status and limitations

Draft 0.5.0 is an early Windows release.

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

## Credits

Draft is designed and built by [Tirup Mehta](https://github.com/TirupMehta).
It is built on **GPUI** by Zed Industries (Apache-2.0), **transcribe.cpp**
(MIT), **Whisper large-v3-turbo** by OpenAI in a GGUF build by handy-computer,
**Instrument Sans** and **Instrument Serif** (OFL 1.1). The full list is in [CREDITS.md](CREDITS.md), and every Rust
dependency is listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

The Credits section is also available in the app, under Settings → Credits,
with links to GitHub and X and a button that opens the installed license files.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a change. It covers the
Windows setup, the design rules, the checks that must pass, and the license.

## License

Draft is released under the [MIT License](LICENSE). The bundled typefaces are
under the SIL Open Font License 1.1. The speech model is distributed with
Draft and is subject to its own terms. See [NOTICE](NOTICE),
[CREDITS.md](CREDITS.md), and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
