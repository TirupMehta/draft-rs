# Draft 0.2.0 for macOS

Private, on-device dictation for Apple Silicon Macs. Hold **Right Ctrl**, speak
naturally, and release. Whisper large-v3-turbo transcribes on Metal, on this Mac.

## Download

- **Draft-0.2.0-macos-arm64.dmg** - macOS 13 or later, Apple Silicon
  (M1/M2/M3/M4). This build is not for Intel Macs.
- The F16 **Whisper large-v3-turbo** model is bundled in the DMG. No model
  download is needed.
- **SHA-256:** attached beside the DMG as
  `Draft-0.2.0-macos-arm64.dmg.sha256`.

## First launch

This is an unsigned preview. It is ad-hoc signed only, not notarized, so macOS
will block the first launch until you allow it:

1. Open the DMG and drag **Draft** to **Applications**.
2. In Applications, right-click **Draft** and choose **Open**, then **Open**
   again. If macOS does not offer Open, use System Settings, Privacy & Security,
   **Open Anyway**.
3. If you prefer Terminal, the equivalent is:
   `xattr -dr com.apple.quarantine /Applications/Draft.app`
4. Grant **Accessibility** and **Input Monitoring** when Draft asks. The exact
   panes are under Settings, Dictation, Permissions. Relaunch after granting.
5. Hold **Right Ctrl**, speak, and release.

Because the signature is ad-hoc, replacing the app with a new build can reset
the privacy grants. Do not move the app after granting them.

## Status

This is a test build, not a verified macOS release:

- CI builds, links, bundles, ad-hoc signs, and verifies the app on Apple
  hardware, and this DMG was produced by that job.
- Nobody has dictated on a physical Mac yet. The event tap, key suppression,
  Escape cancellation, paste, floating HUD, permission onboarding, launch at
  sign-in, and Metal inference are implemented but not runtime-verified.
- Developer ID signing and notarization have not been run. Both need an Apple
  Developer account and repository secrets.
- Updates are manual; the app does not check for updates.
- Windows stays on v0.2.0 for now:
  https://github.com/TirupMehta/draft-rs/releases/tag/v0.2.0

## Privacy

Audio is captured only while the dictation key is held, transcribed on this Mac,
pasted at the cursor, and discarded. No account, no telemetry, no audio upload.
Settings, history, and logs live in `~/Library/Application Support/Draft4`; the
bundled model stays inside the app.

Full credits and third-party notices are in the repository:
[CREDITS.md](https://github.com/TirupMehta/draft-rs/blob/main/CREDITS.md) and
[THIRD_PARTY_NOTICES.md](https://github.com/TirupMehta/draft-rs/blob/main/THIRD_PARTY_NOTICES.md).
The project license is [MIT](https://github.com/TirupMehta/draft-rs/blob/main/LICENSE).
