# macOS

Draft is a Windows app today. This document is the plan for a macOS build, what
is already neutral, what is genuinely missing, and what cannot be confirmed
without an Apple machine.

**No macOS build has been linked, signed, or run.** The two input modules
(`draft-hotkey`, `draft-paste`) now exist and compile for `aarch64-apple-darwin`
with `cargo clippy -D warnings` from this Windows machine, and a `macos-14` job
in `.github/workflows/check.yml` builds the workspace on Apple hardware. That
verifies compilation, not behaviour: nothing below has been exercised by a
person on a Mac, and there is no `.app` to download. Treat every step as
unverified until a Mac produces a passing build and a real dictation.

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
| `apps/desktop` | GPUI 0.2.2 is a macOS-first framework; `tray-icon` supports the menu bar. `window_control.rs` is already written as "no-op away from Windows" with `#[cfg]` seams. |

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

What is still missing here is the permission interface: the app shows the
honest error text from `start()` in its error strip, but there is no button that
opens the exact System Settings pane, and no live "granted / not granted" state.

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

The Windows build pokes `HWND` styles. macOS equivalents:

| Windows | macOS |
|---|---|
| `WS_EX_NOACTIVATE` | `NSWindow` with `canBecomeKeyWindow` overridden to `NO` |
| `WS_EX_TRANSPARENT` | `[window setIgnoresMouseEvents:YES]` |
| `HWND_TOPMOST` | `[window setLevel:NSStatusWindowLevel]` |
| hide to tray | `[window orderOut:nil]` |
| `SW_SHOWNOACTIVATE` for the HUD | `orderFrontRegardless` with `NSWindowCollectionBehaviorCanJoinAllSpaces \| NSWindowCollectionBehaviorFullScreenAuxiliary` so the HUD appears over full-screen apps |
| `DwmSetWindowAttribute` dark title bar | `NSAppearanceNameDarkAqua` on the window |
| `SPI_GETCLIENTAREAANIMATION` | `NSWorkspace.accessibilityDisplayShouldReduceMotion` |

`LSUIElement = true` in `Info.plist` gives a menu-bar-only app with no Dock icon,
which is what the tray-only behaviour is on Windows.

### 2.4 Launch at sign-in

`HKCU\...\Run` has a macOS counterpart in `apps/desktop/src/startup.rs`: a
LaunchAgent plist at `~/Library/LaunchAgents/in.tirup.draft.plist`, written with
`ProgramArguments = [<current exe>, --background]`, `RunAtLoad`, `Aqua` session,
and `Interactive` process type. Enabling has the same meaning as on Windows: it
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
no loose shader files. `ComputeBackend::Metal` already exists; the work is the
build script and confirming a real transcription.

A universal binary (`aarch64-apple-darwin` + `x86_64-apple-darwin`) needs a
fat native library, or two builds shipped as two downloads. Start with Apple
Silicon only; an Intel Mac build is a second release, not a first one.

---

## 3. Staged plan

Each stage ends in something observable. Do not skip a verification.

| Stage | Work | Done when |
|---|---|---|
| 0 | A Mac with Xcode command line tools, Rust, CMake, and the Metal toolchain | `cargo check -p draft-core` passes |
| 1 | **Written.** `crates/draft-hotkey/src/macos.rs`: active tap, keycode table, `FlagsChanged`, Escape, re-enable; `start()` returns a listener whose `Drop` stops the loop | A debug build prints real key events to the log and suppresses Right Control |
| 2 | Accessibility + Input Monitoring prompts, with honest state in the UI: "Grant Accessibility" with a button that opens the exact Settings pane | Denying permission shows a real error, not a silent dead key |
| 3 | **Written.** `crates/draft-paste/src/macos.rs`: Cmd+V from a private source, same modifier wait | A transcript lands in TextEdit at the cursor |
| 4 | `window_control.rs` macOS module: hide/show, HUD level and click-through, reduce-motion | The HUD floats over a full-screen app and never takes focus |
| 5 | **Written (LaunchAgent).** `startup.rs` macOS module | Toggle on, sign out, sign in, Draft is in the menu bar |
| 6 | Metal engine build and a real transcription | `draftctl transcribe bench/fixtures/jfk.wav` returns the verified text |
| 7 | Bundle: `Draft.app`, `Info.plist` (`LSUIElement`, `NSMicrophoneUsageDescription`, `CFBundleIdentifier`), icon set from `draft-256.png` | The `.app` launches from Finder with no terminal |
| 8 | `codesign` with a Developer ID, `notarytool` submit, `stapler staple`, DMG | A downloaded DMG opens without a Gatekeeper block |
| 9 | **Added.** A `macos-14` job in `.github/workflows/check.yml` runs fmt, clippy, test, and a release build | A green run on a machine that is not the author's |

Stages 1 and 3 are code-complete and compile for `aarch64-apple-darwin`, but
their "done when" columns are still unmet: no Mac has run them. The `macos-14`
job verifies compilation and linking on Apple hardware; it cannot grant TCC
permissions, so it cannot verify the tap itself.

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
  `CGEventSourceKeyState`) link; the `macos-14` job answers this one.
- Whether the HUD renders correctly over a full-screen Space.
- Whether `NSStatusWindowLevel` is high enough above other floating panels.
- Whether the Metal backend produces the same transcript as Vulkan.
- Whether the TCC prompts appear, and what they say.
- Whether a notarized DMG opens cleanly on a machine that has never seen Draft.
- Whether the app passes `codesign --verify --deep --strict`.

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
