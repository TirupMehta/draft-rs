# AGENTS.md - Draft (v4)

Authoritative working notes for any agent (or person) changing this repository.
Read this before touching code. It records the decisions that are not obvious
from the diff, the rules that must not be broken, and the exact commands to
build, verify, install, and inspect the result.

The product is described in `README.md`; the product intent is in `docs/SPEC.md`;
the measured performance and the thermal policy are in `docs/PERFORMANCE.md`;
the open work is in `docs/ROADMAP.md`.

---

## 1. What this is

Draft is a private, on-device dictation app for Windows. You hold a key, speak,
release, and the transcript is pasted at your cursor. The model runs locally
(Whisper large-v3-turbo, F16 GGUF, Vulkan). Audio never leaves the machine. A
macOS Apple Silicon preview builds in CI and ships as an ad-hoc-signed DMG, but
it has not been run on a physical Mac; `docs/MACOS.md` is the authoritative
status.

This repository is the **v4 rewrite**. The shipping app is a different codebase
at `C:\all\Draft`.

### Non-negotiable rules

1. **Never touch the shipping app.** `C:\all\Draft` is read-only for this
   project. Do not read, copy, migrate, overwrite, or install anything into it.
2. **Never collide with its data.** All v4 state lives in
   `%LOCALAPPDATA%\Draft4` (see `crates/draft-core/src/paths.rs`). The shipping
   app owns `%LOCALAPPDATA%\Draft`. The two must never share a file, a folder,
   or a registry value.
3. **The interface must be honest.** Never fake a recording, a progress bar, a
   model state, a latency number, or a permission. If the value is not real, do
   not show it.
4. **Privacy copy must be accurate.** Say what is true: audio is captured only
   while the key is held, transcribed locally, and discarded. The one exception
   is the model file, which is installed once (bundled with the release
   installer, or downloaded from the pinned source when building without it) and
   checksum-verified. Do not claim "no network activity" without naming that
   exception.
5. **Power and speed are product features.** Idle must cost nothing. See
   §8 before adding any animation, timer, or repaint.
6. **No new dependencies without a reason in the commit message.** No network
   calls at runtime, no telemetry, no crash reporting, no update pings.
7. **No release test hooks.** Test affordances belong in `tools/` or behind
   `debug_assertions`, not in the shipped path.

---

## 2. Repository map

```
crates/draft-core     settings, history, text cleanup, model catalog, paths
crates/draft-engine   transcribe.cpp GGUF inference (Vulkan/CPU), one engine
crates/draft-audio    cpal capture, 16 kHz resample, live level
crates/draft-hotkey   hold-to-talk key: platform-neutral config, Windows hook,
                      macOS event tap
crates/draft-paste    clipboard insert (arboard), copy_text, Cmd+V or Ctrl+V
crates/draft-cli      the `draftctl` binary
apps/desktop          the GPUI 0.2.2 shell (the product UI)
assets                app icon (draft.ico, draft-256.png) and bundled fonts
bench                 fixtures, baseline data, latency measurements
docs                  SPEC, ROADMAP, PERFORMANCE, MACOS
packaging/windows     Inno Setup installer script
scripts               build-installer.ps1, the release installer builder
tools                 release guards (check-x86-baseline.py)
```

### What is not app code

The shipped product is **one `draft.exe`** (plus `draftctl` for development)
and **one `Draft-Setup-<version>.exe`** installer. The only developer script is
`scripts/build-installer.ps1`, which invokes the Inno Setup compiler to build
that installer. The app itself never calls PowerShell, and no script is
installed with it. Do not add a script for a feature.

The installer copies the bundled model into `%LOCALAPPDATA%\Draft4\models`, not
into the install folder. That entry must keep `uninsneveruninstall`: Inno
otherwise registers every file it copies for deletion, so uninstall would remove
a 1.5 GB model the confirmation dialog promises to keep. Test uninstall after
touching `packaging/windows/draft.iss`.

### The desktop shell (`apps/desktop/src`)

| File | Responsibility |
|---|---|
| `main.rs` | CLI parsing, window creation, the HUD window, the poll loop |
| `app.rs` | `DraftApp`: the phase state machine and the whole render tree |
| `theme.rs` | the design system: palettes, type scale, spacing, `mix()` |
| `motion.rs` | the animation store (hover, press, selection, fades) |
| `hud.rs` | the click-through recording HUD |
| `tray.rs` | tray icon and menu |
| `window_control.rs` | native window bits: hide/show, title-bar appearance, hover |
| `worker.rs` | the engine thread; owns the model for the life of the app |
| `startup.rs` | the per-user `Run` entry for launch at sign-in |

### Event flow

```
keyboard hook ──> hotkey events ──> DraftApp::start_capture / stop_capture
                                          │
                                          v
                             Recorder (cpal) ──> Job{samples, serial, preview, paste}
                                          │
                                          v
                                  worker thread (owns the model)
                                          │
                                          v
                Event::Ready | Partial | Transcribed | Empty | JobFailed
                                          │
                                          v
                          poll() ──> state + HUD + window repaint
```

The UI thread never waits for a model load or a decode. The engine thread owns
the model and runs one inference at a time.

---

## 3. The design system

Everything visual comes from `theme.rs`. Do not hard-code a color, a size, or a
radius in a view.

### Palettes

`Palette::light()` is the default; `Palette::dark()` is a designed second
theme, not an inversion. The active palette follows the `appearance` setting
(`light` by default; `system` follows the OS, including live system changes).

| Token | Dark | Light | Use |
|---|---|---|---|
| `canvas` | `#0f0f12` | `#f4f1ea` | the window background |
| `surface` | `#151519` | `#fcfaf5` | the settings rail, result strips |
| `raised` / `raised_hover` | `#1c1c22` / `#25252d` | `#ece8df` / `#e4dfd4` | control tracks |
| `elevated` | `#2b2b34` | `#ffffff` | selected controls |
| `track` | `#34343e` | `#dcd6cb` | quiet lines, meter baseline |
| `hairline` | 8% warm white | 9% warm black | separators |
| `text` | `#f7f4ee` | `#1b1915` | primary text |
| `secondary` | `#bab6ae` | `#575249` | supporting text |
| `tertiary` | `#918d85` | `#746f66` | metadata |
| `accent` | `#f2b45f` | `#a3610c` | the single accent |
| `danger` | `#ec7062` | `#b23a2e` | errors only |

The mark is the signal-D: three rising bars and a half-disc, drawn from
primitives in `app.rs` and `tray.rs`. It uses the palette's `text` color, so it
stays monochrome in both themes; amber is reserved for live state and
selection. `assets/draft.ico` and `assets/draft-256.png` are generated from the
same geometry; the build script watches the ICO.

There is exactly one accent. If a second hue appears, one of the two is wrong.

### Type

Inter (Regular, Medium, SemiBold) for every interface node, bundled in
`assets/fonts`. JetBrains Mono (Regular, Medium) for the live timer and the
command-line example, where tabular figures matter. Both are registered at
startup so rendering is identical on every machine; sources and the OFL
licenses live next to the files. Sizes in `theme::type_size`:

| Token | Size | Use |
|---|---|---|
| `META` | 12 | metadata, timestamps, paths |
| `SMALL` | 13 | descriptions, controls |
| `BODY` | 14 | row labels, secondary copy |
| `TITLE` | 17 | the wordmark, compact headings |
| `HEADLINE` | 26 | settings section headers |
| `DISPLAY` | 34 | the one hero line on the everyday surface |
| `TIMER` | 48 | the live elapsed time |

Every text node sets an explicit `line_height`. Spacing comes from
`theme::space` (4/8/12/16/24/32). Radii: 6 px controls, 12 px surfaces, full
for pills and switches.

### Layout rules

* **One focal point per view.** The everyday view uses the full window width
  with 32 px gutters: a 340 px instruction column and a 244 px input band.
  The left side carries the status pill, one 34 px hero line, a 56 px keycap
  whose label is the same 34 px, and (while recording) a 48 px timer. The band
  shows the last 1.6 s of real input level, labelled `INPUT LEVEL` / `LIVE` or
  `IDLE`, with a now tick at the right edge. The last transcript and a centered
  privacy line sit below. Configuration lives behind a view switch.
* **No dashboard.** Do not wrap content in bordered panels; use space, a
  hairline, and type weight to build hierarchy.
* **The window is one surface.** The system caption is removed
  (`TitlebarOptions { appears_transparent: true }`) and Draft draws its own
  48 px title bar: brand mark, wordmark, the on-device pill, the
  Dictate/Settings control, then minimize and close. Exactly one wordmark.
* **One window size:** 680x500 client area for both views. The everyday view
  uses the full width with 32 px gutters: a 340 px instruction column and a
  244 px input band. `WINDOW_SIZE` in `app.rs` is the single source. The window
  is not resizable, so fullscreen is not reachable and switching views never
  reallocates the GPU surface.
* **Credits is part of Settings**, not a modal: the author, GitHub and X
  actions, a single Built with panel, and the license. `Open licenses` reveals
  the `licenses` folder installed beside `draft.exe`.
* **The title bar is draggable** from the brand or the empty space between the
  brand and the view control (`WindowControlArea::Drag`). Controls and buttons
  must not sit inside a drag region or they will not receive clicks.

### Controls

| Control | Where | Notes |
|---|---|---|
| `chip()` | hotkey and microphone choices | selected = quiet accent wash |
| `toggle()` | boolean settings | the knob is the only moving part |
| `text_button()` | "Copy", "Open", "Test", "Clear" | a quiet text action |
| `view_tab()` | Dictate / Settings | selection is a separate animated value |
| `section_button()` | settings rail | a 2 px accent bar fades in and out |

All of them are hover- and press-animated through `motion.rs`.

### Motion

`motion.rs` holds one eased 0..1 value per control. It is deliberately tiny:
one number per control, advanced once per poll, and nothing runs while the app
is idle.

* Hover, press, and view selection use it.
* The view and section switches are instant. There is no cross-fade and no
  window resize; the fixed surface keeps text crisp and input immediate.
* Hover is 160 ms and press is 90 ms, so a click answers immediately and a
  hover settles. Both are **zero** when Windows reports that animations are
  off (`SPI_GETCLIENTAREAANIMATION`, read in `window_control.rs`). Respect the
  system setting; do not add a second, parallel motion system.

### States

`Phase` in `app.rs` is the whole truth of what the app is doing:

| Phase | What the everyday view shows |
|---|---|
| `Loading` | "Warming the model" and why |
| `Ready` | "Speak naturally.", a 56 px keycap with a 34 px label, a paste-aware hint, an idle input band |
| `Listening` | "Speak now", a 48 px live timer, the live input band, the cancel hint |
| `Transcribing` | "Finishing up" with a quiet ellipsis, no fake percentage |
| `Error` | plain language plus a recovery action ("Open audio settings") |

The recording meter is a real amplitude history (`VecDeque<f32>`, one sample
per poll, `ENVELOPE_SAMPLES`). It is not a drawn wave, and it must not move
when there is no sound.

---

## 4. Settings

`crates/draft-core/src/settings.rs` owns the schema and an atomic write.
`settings.json` lives in `%LOCALAPPDATA%\Draft4`.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `hotkey` | string | `"right-ctrl"` | one of right-ctrl, right-alt, f8, f9, pause, scroll-lock |
| `model` | path or null | null | null uses the pinned F16 model |
| `microphone` | string or null | null | null follows the system default |
| `paste` | bool | true | insert the transcript at the cursor |
| `live_preview` | bool | false | show partial text while speaking (extra GPU work) |
| `launch_at_startup` | bool | true | start in the tray at sign-in |
| `appearance` | string | `"light"` | `light` (default), `dark`, or `system` |

Missing fields fall back to the defaults, so adding a field is a compatible
change. Unknown fields are ignored.

`paste` is applied per job (`Job.paste`), so the switch takes effect without a
restart. `--no-paste` on the command line turns it off for one launch.

---

## 5. Data and paths

| What | Where |
|---|---|
| Settings | `%LOCALAPPDATA%\Draft4\settings.json` |
| Model | `%LOCALAPPDATA%\Draft4\models\whisper-large-v3-turbo-F16.gguf` |
| History | `%LOCALAPPDATA%\Draft4\history.json` (override with `DRAFT_HISTORY_PATH`) |
| Logs | `%LOCALAPPDATA%\Draft4\logs\draft.log.YYYY-MM-DD` |
| Settings override | `DRAFT_SETTINGS_PATH` |
| Model override | `DRAFT_MODEL_PATH` |

The history store keeps text and duration only - never audio, never a window
title, never a full URL. It is bounded (retention window, entry cap, byte
cap) and a malformed file is quarantined rather than overwritten.

---

## 6. Build, test, install

```text
cd C:\all\draft-rs

cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release

# Install the new build (the installer refuses to run while Draft is open)
Get-Process draft -ErrorAction SilentlyContinue | Stop-Process -Force
Copy-Item target\release\draft.exe dist\draft.exe -Force
.\scripts\build-installer.ps1
Start-Process dist\Draft-Setup-0.2.1.exe -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -Wait
```

Notes:

* `cargo clippy --all-features` fails locally without the CUDA toolkit. The CI
  form above is the one that must stay green.
* The installer is Inno Setup, per-user, `PrivilegesRequired=lowest`, and it
  writes the `HKCU\...\Run\Draft` value with `--background` and removes it on
  uninstall. It needs the Inno Setup 6 compiler (`ISCC.exe`).
* A version bump touches `Cargo.toml` (`[workspace.package] version`) and
  `packaging/windows/draft.iss` (`#define AppVersion`).
* **The Windows CPU floor is AVX2**, pinned in `.cargo/config.toml` as
  `TRANSCRIBE_CMAKE_ARGS=-DGGML_NATIVE=OFF ...`. Do not remove it. ggml builds
  its CPU backend for the build machine by default, so the CI runner's AVX-512
  leaked into the first 0.2.1 installer and `draft.exe` died with
  `0xc000001d` (the faulting instruction was `vmovups zmm0, ...`) on an AVX2
  laptop. `tools/check-x86-baseline.py` decodes `.text` and fails a release
  build above 2,000 zmm instructions; a portable build has about 300.
* `draftctl` subcommands: `clean`, `transcribe`, `bench`, `mic`, `listen`,
  `model status|list|install|verify|remove`, `history list|search|delete|clear`.

### CI

`.github/workflows/check.yml` runs on every push to `main`: `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test
--workspace`, and `cargo build --workspace --release`. Do not weaken it.

`.github/workflows/release-windows.yml` and `release-macos.yml` are manual. They
build the installer or DMG, verify the bundled model, write a SHA-256, and
upload the artifacts. They do not publish; the maintainer creates the release
from those artifacts so it is attributed to their account.

---

## 7. Command-line and environment

App flags: `--model PATH`, `--backend auto|vulkan|metal|cuda|cpu`, `--hotkey
NAME`, `--microphone NAME|default`, `--no-paste`, `--background`, `--settings
PATH`.

`--background` starts the window hidden in the tray; the sign-in Run entry uses
it.

`draft-hotkey` ignores synthetic keyboard input unless
`DRAFT_ALLOW_INJECTED_KEYS` is set, so Draft's own paste chord can never start
a capture. That is the supported way to test the recording state.

---

## 8. Performance and power

* **Idle = zero repaints.** `poll()` notifies only on a real state change.
  Any new animation must not repaint while nothing is happening.
* Repaint budget: event poll 60 Hz, visible window 60 fps while audio is open,
  HUD 60 fps, and a hidden window is not repainted at all.
* Live preview is **off by default**. Each preview is a full GPU pass over the
  warm model; that is what made laptop fans spin. The final pass always uses
  the complete clip.
* The release measurement (see `docs/PERFORMANCE.md`): warm p95 of
  239/234/375/543/1186 ms for 3/5/15/30/60 s of audio, and 264 ms for the JFK
  clip. Real-speech WER parity with v3.5 is still unmeasured and is an open
  release gate.
* One shadow, on the HUD only, because it floats over other applications.
  No blur, no gradients, no large shadow stacks.
* Do not add a dependency for a visual effect. If an effect is not worth a
  dependency, it is not worth shipping.

---

## 9. Visual verification

The repository ships no inspection scripts. For a visual change, capture the
running window with the OS screenshot tool and check both palettes, the fixed
680x500 surface, text clipping, and the HUD. A layout bug is easier to see in a
screenshot than in a diff.

---

## 10. Git
* Branch `main`, pushed directly. The repository is private.
* Commit messages: one line, imperative, describing the change
  (`feat: ...`, `fix: ...`, `perf: ...`). Put the reasoning in the body.
* Push in logical batches. A redesign that changes the shell, the theme, and
  the settings schema is one commit, not three.
* `.commandcode/` and other tool scratch directories are not part of the
  project; do not commit them.

---

## 11. Known gaps
* The 8 px non-client frame GPUI reserves means the content does not reach the
  very edge of the window. Removing it means subclassing the window procedure
  for `WM_NCCALCSIZE`; this was judged too risky to do without a human
  watching the result.
* The settings pane has no keyboard navigation.
* The history pane is not built; the store and `draftctl history` exist.
* Real-speech WER parity with v3.5 is unmeasured.
* The installer is unsigned, so SmartScreen warns on first run.
* macOS has a compiled shell but no verified runtime. `docs/MACOS.md` is the
  staged plan: the active event tap, `Cmd+V` paste, AppKit window behaviour,
  LaunchAgent sign-in, live permission rows, and the `Draft.app` bundle script
  all build on the `macos-14` runner, which also ad-hoc signs the bundle and
  writes a DMG. Nothing has granted TCC permissions, typed a transcript, or run
  a signed build; Developer ID signing and notarization need credentials. Do not
  claim macOS support until the document's stage 8 is done and a person has
  dictated on a signed build.
* Linux remains a stub.
