# AGENTS.md — Draft (v4)

Authoritative working notes for any agent (or person) changing this repository.
Read this before touching code. It records the decisions that are not obvious
from the diff, the rules that must not be broken, and the exact commands to
build, verify, install, and inspect the result.

The product is described in `README.md`; the product intent is in `docs/SPEC.md`;
the measured performance and the thermal policy are in `docs/PERFORMANCE.md`;
the open work is in `docs/ROADMAP.md`. A full handoff brief for a fresh agent
lives in `docs/HANDOFF.md`.

---

## 1. What this is

Draft is a private, on-device dictation app for Windows. You hold a key, speak,
release, and the transcript is pasted at your cursor. The model runs locally
(Whisper large-v3-turbo, F16 GGUF, Vulkan). Audio never leaves the machine.

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
   is the model file, which is downloaded once and checksum-verified. Do not
   claim "no network activity" without naming that exception.
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
crates/draft-hotkey   low-level Windows keyboard hook
crates/draft-paste    clipboard insert (arboard), copy_text
crates/draft-cli      the `draftctl` binary
apps/desktop          the GPUI 0.2.2 shell (the product UI)
assets                app icon (draft.ico, draft-256.png)
bench                 fixtures, baseline scripts, latency measurements
docs                  SPEC, ROADMAP, PERFORMANCE, HANDOFF
packaging/windows     Inno Setup installer script
scripts               build-installer.ps1, setup-windows.ps1
tools/inspect         render-inspection tooling (see §9)
```

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

`Palette::dark()` is the default; `Palette::light()` is a designed second
theme, not an inversion. The active palette follows the `appearance` setting
(`system` follows the OS, including live system changes).

| Token | Dark | Light | Use |
|---|---|---|---|
| `canvas` | `#0d0d0f` | `#faf9f7` | the window background |
| `surface` | `#101014` | `#ffffff` | raised surfaces |
| `elevated` | `#18181e` | `#ffffff` | pressed states |
| `raised` / `raised_hover` | `#1b1b20` / `#24242b` | `#ffffff` / `#f0efeb` | control tracks |
| `track` | `#2a2a32` | `#e4e2dd` | quiet lines, meter baseline |
| `hairline` | 8% white | 9% black | separators |
| `text` | `#f3f3f5` | `#151517` | primary text |
| `secondary` | `#a3a3ac` | `#5c5c65` | supporting text |
| `tertiary` | `#74747e` | `#8b8b93` | metadata |
| `accent` | `#e8a33c` | `#a86510` | the single accent |
| `danger` | `#e2685c` | `#b03a2e` | errors only |

There is exactly one accent. If a second hue appears, one of the two is wrong.

### Type

Segoe UI Variable (Display for headings, Text for body) — it ships with
Windows and needs no bundled asset. Sizes in `theme::type_size`:

| Token | Size | Use |
|---|---|---|
| `META` | 12 | metadata, timestamps, paths |
| `SMALL` | 13 | body, descriptions, controls |
| `BODY` | 14 | row labels, secondary copy |
| `TITLE` | 16 | section titles |
| `HEADLINE` | 21 | the one focal state line |

Every text node sets an explicit `line_height`. Spacing comes from
`theme::space` (8/12/16/24). Radii: 6 px controls, 12 px surfaces, full for
pills and switches.

### Layout rules

* **One focal point per view.** The everyday view has one state line and
  nothing competes with it. Configuration lives behind a view switch.
* **No dashboard.** Do not wrap content in bordered panels; use space, a
  hairline, and type weight to build hierarchy.
* **The window is one surface.** The system caption is removed
  (`TitlebarOptions { appears_transparent: true }`) and Draft draws its own
  44 px title bar: brand mark, wordmark, the Dictate/Settings control, then
  minimize, maximize, close. Exactly one wordmark.
* **Window sizes:** the everyday view is 560x240, settings is 680x500
  (client area). `view_size()` in `app.rs` is the single source.
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

* Hover, press, view selection, and the section dissolve all use it.
* The view switch is a cross-fade: 70 ms out, the window is resized while
  nothing is visible, 140 ms in (`FADE_OUT`, `FADE_IN` in `app.rs`).
* The window resize must go through `Window::resize` so the GPU surface
  follows it. A raw `SetWindowPos` leaves a stale surface.
* Duration is 150 ms, and it is **zero** when Windows reports that animations
  are off (`SPI_GETCLIENTAREAANIMATION`, read in `window_control.rs`). Respect
  the system setting; do not add a second, parallel motion system.

### States

`Phase` in `app.rs` is the whole truth of what the app is doing:

| Phase | What the everyday view shows |
|---|---|
| `Loading` | "Warming the model" and why |
| `Ready` | "Hold [key] and speak" and "Release to paste at the cursor." |
| `Listening` | "Listening", the real level meter, the release hint |
| `Transcribing` | "Transcribing" with a quiet ellipsis, no fake percentage |
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
| `appearance` | string | `"system"` | `system`, `light`, or `dark` |

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

The history store keeps text and duration only — never audio, never a window
title, never a full URL. It is bounded (retention window, entry cap, byte
cap) and a malformed file is quarantined rather than overwritten.

---

## 6. Build, test, install

```powershell
cd C:\all\draft-rs

cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release

# Install the new build (the installer refuses to run while Draft is open)
Get-Process draft -ErrorAction SilentlyContinue | Stop-Process -Force
Copy-Item target\release\draft.exe dist\draft.exe -Force
.\scripts\build-installer.ps1
Start-Process dist\Draft-Setup-0.2.0.exe -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -Wait
```

Notes:

* `cargo clippy --all-features` fails locally without the CUDA toolkit. The CI
  form above is the one that must stay green.
* The installer is Inno Setup, per-user, `PrivilegesRequired=lowest`, and it
  writes the `HKCU\...\Run\Draft` value with `--background` and removes it on
  uninstall. It needs the Inno Setup 6 compiler (`ISCC.exe`).
* A version bump touches `Cargo.toml` (`[workspace.package] version`) and
  `packaging/windows/draft.iss` (`#define AppVersion`).
* `draftctl` subcommands: `clean`, `transcribe`, `bench`, `mic`, `listen`,
  `model status|list|install|verify|remove`, `history list|search|delete|clear`.

### CI

`.github/workflows/check.yml` runs on every push to `main`: `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test
--workspace`, and `cargo build --workspace --release`. Do not weaken it.

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
* Repaint budget: event poll 60 Hz, visible window 30 fps, HUD 15 fps, and a
  hidden window is not repainted at all.
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

## 9. Inspecting the interface without seeing it

The build host cannot view images. The tooling in `tools/inspect/` renders the
real window and reports it as text, which is how layout, spacing, contrast, and
clipping are actually checked here:

| Script | What it does |
|---|---|
| `tools/inspect/Inspect.cs` | pixel analysis: an ASCII luminance grid, ink bounds, best text contrast, edge-clipping check |
| `tools/inspect/ocr.ps1` | Windows OCR: every string with its x/y, so copy and alignment can be verified |
| `tools/inspect/capture.ps1` | capture the running window, print the grid and the OCR |
| `tools/inspect/listen.ps1` | inject the dictation key to capture the real Listening state and the HUD |
| `tools/inspect/Windows.cs` | the small window helpers the scripts share |

```powershell
cd C:\all\draft-rs
.\tools\inspect\capture.ps1          # the running window
.\tools\inspect\capture.ps1 -Start   # start Draft first
.\tools\inspect\listen.ps1           # the recording state and the HUD
```

`listen.ps1` needs Draft to have been started with
`DRAFT_ALLOW_INJECTED_KEYS=1`; the script starts it with that variable when
Draft is not already running, and says so when it is.

The grid uses `*` for mid tones, `#` for bright pixels, `@` for accent pixels,
and `.` for surfaces, so a dark theme prints as mostly `.` with bright text and
a light theme prints as mostly `#`.

When a visual change is made, capture **both** views and read the OCR
positions. A layout bug (clipped text, a control too far from its label, an
empty region) is obvious in the coordinates even when it is not obvious in a
screenshot.

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
* macOS and Linux shells are stubs; Windows x64 is the only supported target.
