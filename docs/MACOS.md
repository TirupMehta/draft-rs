# macOS

Draft is a Windows app today. This document is the plan for a macOS build, what
is already neutral, what is genuinely missing, and what cannot be confirmed
without an Apple machine.

**No macOS build has been signed with a Developer ID, notarized, or used by a
person on a Mac.** The `macos-14` job in `.github/workflows/check.yml` compiles
the workspace, links the Metal engine, builds `Draft.app`, ad-hoc signs it, and
writes a DMG. That verifies compilation and bundle structure, not behaviour:
nothing below has been exercised with a real keyboard, TCC grants, or a
downloaded build. Treat every runtime claim as unverified until a signed build
dictates on a Mac.

---

## 1. What already carries over

The product is deliberately split so the interesting parts are not Windows
code.

| Piece | State |
|---|---|
| `draft-core` settings, history, text cleanup, model catalog, paths | Platform-neutral. `paths.rs` already resolves `~/Library/Application Support/Draft4` through `dirs`, and `data_dir_label()` prints that form on macOS instead of a Windows variable. |
| `draft-engine` | `ComputeBackend` already includes `Metal`, and transcribe.cpp / ggml ships a Metal backend. The FFI does not need a new abstraction, only a Metal build of the native library. |
| `draft-audio` | `cpal` has a CoreAudio host. Device enumeration, 16 kHz resample, and the live level are already written against `cpal`, not WASAPI. |
| `draft-hotkey` | Written. `macos.rs` holds an active `CGEventTap` at the HID point, a keycode table, `FlagsChanged` handling for the bare modifiers, Escape cancellation, and tap re-enable. Compiled for Apple Silicon; never run. |
| `draft-paste` | Written. `macos.rs` posts `Cmd+V` from a private event source and waits for modifiers with `CGEventSourceKeyState`. Compiled for Apple Silicon; never run. |
| `apps/desktop` | GPUI 0.2.2 is a macOS-first framework; `tray-icon` supports the menu bar. `window_control.rs` now has an AppKit module: hide/show, HUD level, click-through, all Spaces, and reduce-motion. `startup.rs` writes a LaunchAgent. Neither has run. |

The refactor that made `HotkeyConfig` platform-neutral is the only code change
made for macOS so far. It is on `main` and Windows still passes its tests.

---

## 2. What remains

Two modules are written and compile; the rest of the shell, the bundle, and
every runtime question are still open.

### 2.1 The hotkey tap

`crates/draft-hotkey/src/macos.rs` is an active `CGEventTap` at
`kCGHIDEventTap`, inserted at the head, observing `KeyDown`, `KeyUp`, and
`FlagsChanged`. It runs on its own thread with a `CFRunLoop`, pushes into a
bounded channel, and stops by stopping that loop. `start()` returns only after
the loop is running (a zero-length run-loop timer sends readiness from inside
it), so a listener dropped immediately after creation cannot miss its stop.

Keycodes (Carbon `HIToolbox/Events.h`):

| `HotkeyConfig` | macOS keycode | Notes |
|---|---|---|
| `RightCtrl` | `kVK_RightControl` `0x3E` | default |
| `RightAlt` | `kVK_RightOption` `0x3D` | the Mac equivalent of AltGr |
| `F8` | `kVK_F8` `0x64` | |
| `F9` | `kVK_F9` `0x65` | |
| `Pause`, `ScrollLock` | none | Apple keyboards have no such keys, so `HotkeyConfig::available()` returns four choices on macOS |

Four macOS details the module encodes, each of which needs a real keyboard to
confirm:

1. **Modifiers are `FlagsChanged`.** Right Control and Right Option never
   produce `KeyDown`/`KeyUp`. The right and left keys share one flag, so the
   module identifies the key by keycode and derives press/release by toggling
   its own record. Reading the flag instead would miss the release when the
   other side is still held.
2. **Deleting an event.** The Windows hook returns `1` to swallow the key.
   `core-graphics` 0.24 turns a `None` callback result into the original event,
   so `swallow()` sets the returned event's type to `kCGEventNull` instead.
   **This is the least certain line in the port.** If macOS does not discard a
   null-type event, the dictation key and Escape reach the focused app; dictation
   still works, but the key is no longer exclusive. Check it first on a Mac.
3. **Synthetic input.** A real key arrives at the tap with
   `kCGEventSourceStateID == kCGEventSourceStateHIDSystemState`. The module
   ignores anything else unless `DRAFT_ALLOW_INJECTED_KEYS` is set, which keeps
   Draft's own paste chord and other automation from starting a capture. The
   check fails closed: if real events do not report the HID source state, the
   key does nothing, which is visible immediately rather than silently unsafe.
4. **F8 and F9 may be media keys.** Without "Use F1, F2, etc. as standard
   function keys" in System Settings, the top row is intercepted by the media
   layer and the tap never sees `KeyDown`. The two modifier choices are the
   reliable defaults; the function keys are offered because people do enable
   standard function keys, and the log line names the keycode when the tap sees
   nothing.

Two permissions are required, and they are **different**:

1. **Accessibility** (`kAXTrustedCheckOptionPrompt`) to modify or swallow
   events. Without it the tap is read-only and Draft cannot suppress the key.
2. **Input Monitoring** (`IOHIDRequestAccess`) to observe key events at all on
   recent macOS versions.

Both are granted per-app in System Settings → Privacy & Security, both need the
app to be signed and stable in its path, and both require a relaunch. An
unsigned or moved binary loses the grant silently, which is why signing has to
land before any of this is testable by a normal person.

macOS disables a tap whose callback takes too long (`kCGEventTapDisabledByTimeout`)
or when input is blocked (`kCGEventTapDisabledByUserInput`). Both arrive as
events; the callback re-enables the tap and logs a warning. A tap left disabled
looks exactly like a key that stopped working, which is the failure this app
must never present silently.

The permission interface is now in Settings → Dictation on macOS. Every render
reads `AXIsProcessTrusted` and `IOHIDCheckAccess` directly, so a revoked grant
cannot keep showing "Granted", and each row opens the exact pane
(`Privacy_Accessibility` / `Privacy_ListenEvent`) through `/usr/bin/open`.
`Try again` restarts the tap without a relaunch, and the error surface points at
the privacy pane instead of the audio pane when the tap is what failed. Input
Monitoring is requested once through `IOHIDRequestAccess`; after that, opening
the pane is the reliable path. The code compiles on the macOS runner. Nobody has
clicked it on a Mac, granted either permission, or confirmed the panes open.

### 2.2 Paste

Written in `crates/draft-paste/src/macos.rs`. `Cmd+V`, not `Ctrl+V`, is posted
with `CGEventCreateKeyboardEvent` at `kCGHIDEventTap` from a **private** event
source, so Draft's own tap ignores it. The event carries
`kCGEventFlagCommand`; down and up are 25 ms apart because some editors miss a
chord posted in one tick. The modifier-release wait from the Windows module
ports directly through `CGEventSourceKeyState`, checking both sides of Command,
Shift, Option, and Control. The clipboard is restored only if it still holds
the transcript.

Secure event input (a password field) blocks synthetic paste. Nothing is lost:
the transcript stays in the clipboard, and the module reports success, the same
way the Windows path does when an application ignores the chord.

### 2.3 Window behaviour

The Windows build pokes `HWND` styles; `apps/desktop/src/window_control.rs` now
has a `#[cfg(target_os = "macos")]` module that does the same through AppKit.
The handle passed around the app is the `NSView` pointer from
`raw_window_handle`, converted back to its `NSWindow` with `NSView::window()`.

| Windows | macOS, as written |
|---|---|
| `WS_EX_TRANSPARENT` | `setIgnoresMouseEvents(true)` |
| `HWND_TOPMOST` | `setLevel(25)`, the value of `NSStatusWindowLevel` |
| hide to tray | `orderOut(None)` |
| `SW_SHOWNOACTIVATE` for the HUD | `orderFrontRegardless()`, which shows without activating Draft |
| HUD over a full-screen Space | `CanJoinAllSpaces \| FullScreenAuxiliary \| Stationary \| IgnoresCycle` |
| `DwmSetWindowAttribute` dark title bar | nothing: the window has no native title bar |
| `SPI_GETCLIENTAREAANIMATION` | `NSWorkspace.accessibilityDisplayShouldReduceMotion()`, inverted |

`setHidesOnDeactivate(false)` and `setCanHide(false)` keep the HUD on screen
when another application takes focus, which is the whole point of a HUD.

Showing the main window also calls `activateIgnoringOtherApps(true)`: a
menu-bar application is not active when its menu is clicked, so ordering the
window front does not give it the keyboard. That selector is deprecated in
macOS 14 in favour of `activate()`, but it still exists, and `activate()` would
crash on anything older. When the deployment target is 14 this can be a
one-line change.

`LSUIElement = true` in `Info.plist` gives a menu-bar-only app with no Dock icon,
which is what the tray-only behaviour is on Windows. The module does not change
the activation policy at runtime, so until the bundle exists a `cargo run` build
still shows a Dock icon.

What is unverified is everything visual: whether level 25 clears other floating
panels, whether the HUD survives a Space switch, and whether GPUI's own window
bookkeeping tolerates a window ordered out behind its back. GPUI tracks
visibility internally; the Windows path has the same shape, so the risk is known
rather than new.

### 2.4 Launch at sign-in

`HKCU\...\Run` has a macOS counterpart in `apps/desktop/src/startup.rs`: a
LaunchAgent plist at `~/Library/LaunchAgents/in.tirup.draft.plist`, written with
`ProgramArguments = [<current exe>, --background]`, `RunAtLoad`, `Aqua` session,
and `Interactive` process type. Inside a `.app`, the plist uses
`[/usr/bin/open, -a, <Draft.app>, --args, --background]` instead: launching
`Contents/MacOS/draft` directly skips LaunchServices and leaves the app with the
wrong activation policy and no menu-bar identity. Both forms are covered by
unit tests that run on the macOS runner. Enabling has the same meaning as on Windows: it
applies at the next sign-in. Disabling removes the plist. A path containing `&`
is escaped; there is a unit test for it.

`SMAppService.mainApp.register()` is still the preferred API once a signed
`.app` exists in stage 7, because it is what System Settings → Login Items
shows as a first-class app rather than a background item. The fallback was
chosen first because it needs no bundle and the current build has none.

The setting must still be a mirror of the system state, not a private flag: if
the person removes Draft from Login Items in System Settings, the toggle should
show off on next launch. Neither the Windows nor the macOS module reads the
platform back yet; both trust `settings.json`. That is an existing gap, not a
new one.

### 2.5 The engine

`transcribe.cpp` needs a Metal build: `GGML_METAL=ON`, `GGML_METAL_EMBED_LIBRARY=ON`
so the `.metallib` is inside the binary and the app stays a single bundle with
no loose shader files. `ComputeBackend::Metal` already exists.

The first `macos-14` run answered the build question in the least convenient
way: `transcribe-cpp-sys` was configured with `-DTRANSCRIBE_VULKAN=ON` and CMake
stopped at `Could NOT find Vulkan (missing: Vulkan_LIBRARY Vulkan_INCLUDE_DIR
glslc)`. The cause was `draft-engine`'s `default = ["vulkan"]`, which is correct
on Windows and impossible on macOS. The backend is now named by the target
dependency table instead:

```toml
[target.'cfg(target_os = "windows")'.dependencies]
transcribe-cpp = { ..., features = ["vulkan"] }

[target.'cfg(target_os = "macos")'.dependencies]
transcribe-cpp = { ..., features = ["metal"] }
```

`draft-engine`'s `default` feature is now empty, so a plain `cargo build` gets
the platform's backend without a flag a person has to remember. `--features
cuda` and `--no-default-features` still work for the explicit cases. The
`macos-14` job is what proves the Metal library builds; it cannot prove that a
transcript is correct.

A universal binary (`aarch64-apple-darwin` + `x86_64-apple-darwin`) needs a
fat native library, or two builds shipped as two downloads. Start with Apple
Silicon only; an Intel Mac build is a second release, not a first one.

### 2.6 The `.app` bundle

`bash scripts/build-macos-app.sh` assembles `dist/Draft.app` from the release
binary. It writes `Contents/Info.plist` from `packaging/macos/Info.plist`
(`LSUIElement`, `NSMicrophoneUsageDescription`, bundle id `in.tirup.draft`,
minimum macOS 13), derives `Draft.icns` from `assets/draft-256.png`, and copies
the license texts to `Contents/Resources/licenses`, where codesign seals them.
`open_licenses()` still checks beside the executable first, then that Resources
folder on macOS.

Set `MODEL_PATH=/path/to/whisper-large-v3-turbo-F16.gguf` to bundle the model in
`Contents/Resources/models`; `main.rs` reads it there when the data folder has
none, so the first dictation needs no network. The `.github/workflows/release-macos.yml`
workflow downloads the pinned artifact, verifies its SHA-256, builds the DMG,
writes a checksum, and publishes it to a prerelease.

The default signature is ad-hoc (`-`), which is what the macOS runner can do
without secrets. Set `CODESIGN_IDENTITY="Developer ID Application: ..."` to sign
with a real certificate. `MAKE_DMG=1` writes
`dist/Draft-<version>-macos-<arch>.dmg` with an Applications symlink.
Notarization is not automated and has not been run: it needs an Apple Developer
account, a `notarytool` keychain profile, and the same Developer ID. See stage 8.

---

## 3. Staged plan

Each stage ends in something observable. Do not skip a verification.

| Stage | Work | Done when |
|---|---|---|
| 0 | A Mac with Xcode command line tools, Rust, CMake, and the Metal toolchain | `cargo check -p draft-core` passes |
| 1 | **Written.** `crates/draft-hotkey/src/macos.rs`: active tap, keycode table, `FlagsChanged`, Escape, re-enable; `start()` returns a listener whose `Drop` stops the loop | A debug build prints real key events to the log and suppresses Right Control |
| 2 | **Written.** Live permission state in Settings → Dictation; each row opens the exact pane; `Try again` restarts the tap; the error surface routes a tap failure to Privacy | A Mac grants and revokes each permission, the row follows, and denying shows a real error rather than a silent dead key |
| 3 | **Written.** `crates/draft-paste/src/macos.rs`: Cmd+V from a private source, same modifier wait | A transcript lands in TextEdit at the cursor |
| 4 | **Written (unverified).** `window_control.rs` macOS module: hide/show, HUD level and click-through, reduce-motion | The HUD floats over a full-screen app and never takes focus |
| 5 | **Written (LaunchAgent).** `startup.rs` macOS module, bundle-aware through `/usr/bin/open -a` | Toggle on, sign out, sign in, Draft is in the menu bar |
| 6 | Metal engine build and a real transcription | `draftctl transcribe bench/fixtures/jfk.wav` returns the verified text |
| 7 | **Written.** `scripts/build-macos-app.sh` assembles `Draft.app` from `packaging/macos/Info.plist`, the icon, and the licenses; CI ad-hoc signs it and writes a DMG | The CI bundle launches from Finder with no terminal (device check pending) |
| 8 | Documented; ad-hoc signing and DMG creation run in CI. Developer ID, `notarytool`, and stapling need an Apple account and secrets | A downloaded DMG opens without a Gatekeeper block |
| 9 | **Green on `b995e7e`.** The `macos-14` job runs fmt, clippy, test, a release build, the bundle, an ad-hoc codesign verify, and a DMG | A green run on a machine that is not the author's |

Stages 1, 3, 4, 5, and 7 are code-complete and compile for
`aarch64-apple-darwin`, but their "done when" columns are still unmet: no Mac
has run them. The `macos-14` job verifies compilation, linking, and bundle
structure on Apple hardware; it cannot grant TCC permissions, so it cannot
verify the tap, the permission rows, or the HUD.

Stage 8 is not optional polish. TCC permissions are tied to the code signature;
an unsigned build loses Accessibility on every rebuild, which makes stages 1
through 5 untestable in any repeatable way.

---

## 4. What cannot be verified from Windows

Stated plainly, because the interface rules in `AGENTS.md` apply to
documentation too:

- Whether the event tap actually receives Right Control.
- Whether returning a null-type event deletes the dictation key, or lets it
  through.
- Whether hardware events report `kCGEventSourceStateHIDSystemState`; if they do
  not, the synthetic-input guard silently disables the hotkey.
- Whether the zero-length timer fires before `CFRunLoopRun` settles, so
  `start()` returns at the right moment.
- Whether F8 and F9 reach the tap with the default media-key behaviour.
- Whether the two hand-declared symbols (`CGEventTapEnable`,
  `CGEventSourceKeyState`) and the permission symbols link. **Answered:** the
  green `macos-14` run links them.
- Whether the HUD renders correctly over a full-screen Space.
- Whether `NSStatusWindowLevel` is high enough above other floating panels.
- Whether the Metal backend produces the same transcript as Vulkan.
- Whether the TCC prompts appear, what they say, and whether the permission rows
  open the exact panes.
- Whether `open -a Draft.app --args --background` leaves Draft in the menu bar
  after sign-in.
- Whether a notarized DMG opens cleanly on a machine that has never seen Draft.
- Whether a Developer ID signature verifies. **Partly answered:** the CI run
  proves `codesign --verify --deep --strict` passes for the ad-hoc bundle.

No screenshot, latency number, or "works on macOS" claim should appear in the
README, the site, or a release note until stage 9 is green and a person has used
the signed build.

---

## 5. Things that will not port cleanly

- **`DRAFT_ALLOW_INJECTED_KEYS`.** The Windows test hook keys off
  `LLKHF_INJECTED`. macOS synthetic events carry
  `kCGEventSourceStateHIDSystemState` versus `kCGEventSourceStatePrivate`, so the
  equivalent check is different and must be written carefully: a test hook that
  fails open would let Draft's own paste chord start a capture on macOS.
- **The 8 px non-client frame.** GPUI reserves it on Windows; the macOS window
  has no such frame, so the layout may gain 8 px of usable width. Check the
  680×500 surface against the design rules rather than assuming it is identical.
- **Scroll Lock, Pause, AltGr.** No equivalents. The settings list shrinks; do
  not invent a mapping.
- **The model path in copy.** `data_dir_label()` already returns
  `~/Library/Application Support/Draft4`, but every screenshot and the Privacy
  pane need a macOS capture before the copy is called verified there.

---

## 6. Order of work, if only one thing is possible

Stage 1 and stage 8 first, out of order. A tap that works only on the author's
machine, and a signature that keeps permissions across rebuilds, are the two
things that decide whether the rest is a week or a month. Everything else is
mechanical once those two are settled.
