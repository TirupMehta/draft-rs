## Draft 0.5.1

A quieter listening popup.

- **A simpler recording HUD.** The popup is now one small pill: a dot, the live input meter, and the length of the take. The title, the hint and the ESC key cap are gone. It widens only when live preview has words to show, and while the model works it shows three dots and "Transcribing".
- **No more stray rectangle.** On Windows 11 the HUD drew a dark frame and shadow around its whole transparent window, which showed up as a rectangle behind the pill. It is gone.

Free and MIT licensed. Windows 10/11 x64.

**Not code-signed yet**, so Windows SmartScreen may warn: choose More info, then Run anyway. Verify the SHA-256 files first. The standard installer (about 11 MB) downloads your model once on first launch; `Draft-Setup-0.5.1-offline.exe` (1.43 GB) bundles the large model and needs no download.
