# Draft v4 — handoff brief

> **`AGENTS.md` in the repository root is the working agreement for this
> project**: the rules, the architecture, the design tokens, the build and
> install commands, and the inspection tooling. Read that first; this brief
> adds the history and the reasoning behind it.

Copy everything below into the next agent's prompt. It is written to be
self-contained: paths, commands, current state, decisions, and the exact next
steps.

> **Status update:** the redesign described in section 6 has been implemented,
> built, installed, and verified. The remaining work is the polish list in
> section 10. The temporary inspection hooks described in section 5 have been
> removed; the inspection tools in section 8 remain in `tools/inspect/` and
> still work, but they now drive the app through the tray instead of the
> removed environment variables.

---

## 1. The task

Redesign **Draft**, a private Windows on-device dictation app written in Rust,
into a premium product. The user (owner) is the reviewer and has rejected two
earlier attempts. The latest feedback, verbatim:

> "see bro i don't like this - i want seamless body and app look with clean
> text. all current content inside the app also feels very cheap and low
> quality. see how seamless app looks, see how clean and quality font and design
> looks. i want this"

The two reference screenshots the user supplied show:

* **Image 1 (rejected Draft window):** a small window with a *native Windows
  title bar* ("Draft" + minimize/maximize/close) stacked on top of an *in-app
  header row* that repeats "Draft" and shows "Dictate | Settings" as plain
  floating text. The user reads this double header as cheap.
* **Image 2 (reference, a Zed-style app):** one continuous surface. A single
  custom title bar spans the whole window, carrying the app icon, a title, and
  window controls; the body flows straight out of it with a single hairline
  separator. Type is large, calm, and well-hierarchied.

**Required outcome:** one seamless surface (no stacked native title bar + app
header), clean high-quality typography, and content that feels designed rather
than assembled. The product must stay fast, smooth, and power-efficient —
those are hard constraints, not trade-offs.

## 2. Repository and machine

* Repo: `C:\all\draft-rs` (private GitHub `TirupMehta/draft-rs`, branch `main`).
* Last pushed commit: `13991c3 feat: start Draft in the tray at sign-in`.
  CI workflow: `.github/workflows/check.yml` (fmt, clippy `-D warnings`,
  `cargo test --workspace`, `cargo build --workspace --release`).
* **Never touch `C:\all\Draft`** — that is the shipping v3.5 app. Draft v4 data
  lives in `%LOCALAPPDATA%\Draft4`; settings/logs/models/history never overlap.
* Installed copy: `%LOCALAPPDATA%\Programs\Draft\draft.exe`.
* Installer: `dist\Draft-Setup-0.2.0.exe`, built by `scripts\build-installer.ps1`
  (Inno Setup, per-user, `PrivilegesRequired=lowest`).
  Silent install: `Start-Process dist\Draft-Setup-0.2.0.exe -ArgumentList
  '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -Wait`.
  The installer refuses to overwrite while Draft is running (exit code 5), so
  `Get-Process draft | Stop-Process -Force` first.
* Model: `%LOCALAPPDATA%\Draft4\models\whisper-large-v3-turbo-F16.gguf`
  (F16 GGUF, Vulkan). Benchmark-only Q8_0 lives at `C:\all\models\`.
* **This machine has no internet access** (verified: HTTPS to GitHub times out),
  so no new typeface can be downloaded and no new crate can be fetched.

## 3. Architecture (do not disturb the engine)

Rust workspace, edition 2024, `rust-version = 1.85`.

| Crate | Role |
|---|---|
| `crates/draft-core` | settings, history store, text cleanup, model catalog, paths |
| `crates/draft-engine` | `transcribe.cpp` GGUF inference (Vulkan), one engine thread |
| `crates/draft-audio` | cpal capture, 16 kHz resample, live `Recorder::level()` |
| `crates/draft-hotkey` | low-level Windows keyboard hook, `right-ctrl` default |
| `crates/draft-paste` | clipboard insert + `copy_text`, arboard |
| `crates/draft-cli` | `draftctl` (model install, history, benchmark) |
| `apps/desktop` | the GPUI 0.2.2 shell (window, tray, HUD, settings) |

Desktop shell files: `main.rs` (window + CLI + HUD window), `app.rs` (the one
window: state machine + all rendering), `theme.rs` (design tokens),
`hud.rs` (click-through recording HUD), `tray.rs`, `window_control.rs`
(native window bits), `worker.rs` (engine thread), `startup.rs` (HKCU Run entry
for launch at sign-in).

`app.rs` owns: `DraftApp` (phase, hotkey, recorder, jobs, history, tray,
settings) and the whole render tree.

## 4. Work already completed and pushed

* F16 model, Vulkan, warm engine. Release p95 (3/5/15/30/60 s):
  239/234/375/543/1186 ms. JFK 264 ms.
* Live preview is **opt-in** (off by default) because each preview pass is a
  full GPU pass and made fans spin.
* Quiet rendering: HUD repaints at 15 fps, visible main window at 30 fps, a
  hidden window is not repainted at all, the event poll stays at 60 Hz for
  hotkey latency, previews are throttled (next gap = 3x pass time, clamped
  1200-3000 ms).
* Launch at sign-in: `settings.launch_at_startup` (default true) projected onto
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value `Draft` =
  `"<exe>" --background`; the installer writes the same value and removes it on
  uninstall; the app rewrites it after an in-place update.
* Version bumped to **0.2.0** (workspace + Inno `AppVersion`).
* Tests: 44 in `draft-core`, 3 in `draft-audio`. Local gate is green.

## 5. Current uncommitted state (this redesign)

`git status` is dirty. Uncommitted work includes:

* `crates/draft-core/src/settings.rs`: new `appearance: String` field
  (`system` | `light` | `dark`), default `"system"`.
* `apps/desktop/src/theme.rs`: rewritten as a real design system —
  `Appearance` enum, `Palette` struct with **two** intentional palettes
  (`Palette::dark()`, `Palette::light()`), `space`, `type_size`, `radius`
  modules, `display_font()` / `text_font()` (Segoe UI Variable Display / Text),
  `hud_shadow()`.
* `apps/desktop/src/app.rs`: rewritten rendering.
  * `enum View { Dictate, Settings }`, `enum Section { Dictation, Audio,
    Recognition, Appearance, Privacy, Advanced }`.
  * `dictate_view` (focal state, last transcript, privacy line, engine line),
    `settings_view` (rail + pane), per-section row builders.
  * `state_block` renders Ready / Loading / Listening / Transcribing / Error
    honestly. The Listening meter is a **real** amplitude history
    (`VecDeque<f32>`, `ENVELOPE_SAMPLES = 48`, pushed in `poll`).
  * `palette_for(window)` follows the OS in `System` mode.
* `apps/desktop/src/hud.rs`: rewritten HUD (fixed 560 px card, real level
  bars, state label, ESC chip).
* `apps/desktop/src/window_control.rs`: `set_title_bar_dark(hwnd, dark)` via
  `DwmSetWindowAttribute`.
* `apps/desktop/Cargo.toml`: added the `Win32_Graphics_Dwm` feature.
* `apps/desktop/src/main.rs`: `--background` starts hidden in the tray;
  window sizes are 560x240 (Dictate) and 680x500 (Settings).
* `crates/draft-paste/src/lib.rs`: new `copy_text`.

**Temporary inspection hooks that MUST be removed before committing:**
`fn initial_view()` / `fn initial_section()` in `app.rs` read
`DRAFT_VIEW` / `DRAFT_SECTION`, and `main.rs` sizes the window from
`DRAFT_VIEW`. Delete both, restore the plain `View::Dictate` /
`Section::Dictation` initialisers.

## 6. What the design must become next

### 6.1 Seamless chrome (the user's main complaint)

GPUI 0.2.2 already supports this on Windows:

* `WindowOptions.titlebar = Some(TitlebarOptions { title: Some("Draft".into()),
  appears_transparent: true })` — `appears_transparent` makes GPUI create the
  window **without** `WS_CAPTION`, so the client area covers the whole window
  and the native title bar disappears (`platform/windows/window.rs`, the
  `hide_title_bar` flag).
* `gpui::WindowControlArea` (`Drag`, `Min`, `Max`, `Close`) plus the element
  method `.window_control_area(area)`: GPUI's Windows `WM_NCHITTEST` handler
  maps `Drag` to `HTCAPTION` (drag the window), `Min`/`Max`/`Close` to the
  matching `HT*` codes.
* `Window::minimize_window()`, `Window::zoom_window()`, `Window::is_maximized()`
  exist for app-side handling of the buttons.

Plan: draw one 40-ish px bar as the first row of the app content — brand mark +
"Draft" on the left, a **segmented** Dictate/Settings control in the middle or
right, and minimize / maximize / close drawn as minimal line icons on the far
right — then a single hairline, then the body. Remove the separate in-app
"Draft" header row so the wordmark appears exactly once.

Note: with no caption, Windows may reserve a thin resize border; check the
rendered result with the inspector below.

### 6.2 Type

* No font can be downloaded. The system has **Segoe UI Variable Display/Text**,
  Bahnschrift, Cascadia Mono/Code, Consolas, Calibri, Candara, Corbel, Georgia.
* Current type scale is too small and is a big part of the "cheap" feeling.
  Raise it: body 14, small 13, meta 12, section title 16, headline 20-22, with
  explicit `line_height` on every text node.
* Use `Segoe UI Variable Text` for body/UI and `Segoe UI Variable Display` for
  headings, and `Cascadia Mono` for paths/technical metadata where it reads
  better.

### 6.3 Content

Keep every existing capability (shortcut, microphone + level test, model path,
live preview, paste-at-cursor, launch at sign-in, theme, history clear, model
install info, logs, data folder). Settings are grouped by intent, advanced
details live in **Advanced**, and privacy copy is factual:

> "Your voice stays on this device." / "Draft captures audio only while you
> hold the dictation key. The clip is transcribed on this machine and then
> discarded." / "The one exception is the model file: it is downloaded once
> from the internet and checked against a pinned checksum. The app makes no
> network requests." / "Finished dictations are kept in a local history file so
> you can search them later. That file is never uploaded or synced."

Never fake recording, progress, model status, or benchmarks. The only real
numbers available are `engine_ms`, `audio_s`, and `load_ms`.

## 7. Performance rules (do not regress)

* Idle window = **zero** repaints. `poll()` only notifies on state change.
* HUD 15 fps, visible window 30 fps, event poll 60 Hz.
* No continuous animation, no blur, no gradients, no large shadow stacks.
  One small shadow on the HUD is acceptable (it floats over other apps).
* The transcribing ellipsis animates only while a job is pending (brief).
* Keep the HUD's repaint at 15 fps; the meter must be driven by the real
  `Recorder::level()`.
* No new dependencies; no network calls at runtime.

## 8. How to inspect the rendered result

`tools/inspect/` (copied from the working scripts) renders the real window and
reports it as text, because this workflow cannot see images:

* `tools/inspect/Inspect.cs` — pixel analysis: luminance grid, ink bounds,
  best text contrast, edge-clipping check.
* `tools/inspect/ocr.ps1` — Windows OCR; prints every string with its x/y,
  which is how spacing, alignment, and copy are verified.
* `tools/inspect/shot.ps1` — starts Draft with `DRAFT_VIEW` / `DRAFT_SECTION`,
  captures the window, prints the grid and OCR.
* `tools/inspect/listen.ps1` — injects the dictation key (with
  `DRAFT_ALLOW_INJECTED_KEYS`, an existing documented test hook in
  `draft-hotkey`) to capture the real Listening state and the HUD.
* `tools/inspect/capture2.ps1` — generic capture by window handle.

Typical loop:

```powershell
cd C:\all\draft-rs
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p draft-desktop --release
Copy-Item target\release\draft.exe dist\draft.exe -Force
Get-Process draft -ErrorAction SilentlyContinue | Stop-Process -Force
.\scripts\build-installer.ps1
Start-Process dist\Draft-Setup-0.2.0.exe -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -Wait
& "$env:TEMP\opencode\ocr.ps1" -Path "$env:TEMP\draft-dictate-dictation.png"
```

## 10. What has been built since the brief

* **Seamless chrome.** `TitlebarOptions { appears_transparent: true }` removes
  the system caption; Draft draws its own 44 px title bar (brand, wordmark,
  Dictate/Settings control, minimize, maximize, close) with a hairline under
  it. The window is dragged from the brand or the empty title-bar space using
  `WindowControlArea::Drag`.
* **Type scale raised** to 12/13/14/16/21 with explicit line heights, and the
  body and display faces are Segoe UI Variable Text and Display.
* **Two views** — Dictate (560x240) and Settings (680x500) — switched with a
  cross-fade: the old view fades out (70 ms), the window is resized through
  `Window::resize` while nothing is visible, and the new view fades in
  (140 ms). The resize goes through the framework so the GPU surface follows.
* **`motion.rs`** is a small animation store: one eased 0..1 value per control
  for hover, press, view selection, and the section dissolve. It respects
  `SPI_GETCLIENTAREAANIMATION`, so Windows' "animation effects" setting turns
  every transition off.
* **Sections** are grouped by intent with an animated selection bar, and a
  section change dissolves the pane.
* **Light and dark palettes**, both deliberate, with the native frame told
  which appearance to use through `DwmSetWindowAttribute`.

## 11. Still open

* The 8 px non-client frame is GPUI's own inset, so the content does not reach
  the very edge of the window. Removing it means subclassing the window
  procedure for `WM_NCCALCSIZE`; that was judged too risky for now.
* The settings pane has no keyboard navigation yet.
* The history pane is still missing.

## 9. Definition of done

1. One seamless surface: no native title bar stacked above an app header, one
   wordmark, one hairline, content that flows out of the title bar.
2. Typography that reads as designed: larger scale, explicit line heights, real
   weight contrast, no floating tiny labels.
3. Every state honest: Ready, Loading, Listening (real meter), Transcribing,
   Completed, Error with a recovery action.
4. Settings grouped by intent, advanced details preserved but out of the way.
5. Dark and light palettes both intentional; the native frame follows the
   chosen theme.
6. Idle power unchanged: zero repaints when nothing is happening.
7. `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, and a release build all pass; the installer is
   rebuilt; the app is installed and verified; the work is committed and pushed.
