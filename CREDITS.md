# Credits

Draft is designed and built by **Tirup Mehta**.

- GitHub: [TirupMehta](https://github.com/TirupMehta)
- X: [@TirupMehta](https://x.com/TirupMehta)
- Website: [tirup.in](https://tirup.in)
- Project: [TirupMehta/draft-rs](https://github.com/TirupMehta/draft-rs)

Draft stands on open software and open type. Thank you to the people who made
these projects.

## Interface

**GPUI** — the native, GPU-rendered interface toolkit used by Draft.
Copyright Zed Industries. Licensed under the Apache License, Version 2.0.
[Source](https://github.com/zed-industries/zed/tree/main/crates/gpui)

## Inference

**transcribe.cpp** — the C/C++ speech-to-text engine behind Draft's local
Whisper inference. The Rust bindings used here are the `transcribe-cpp` crate.
Licensed under the MIT License.
[Source](https://github.com/handy-computer/transcribe.cpp)

**Whisper large-v3-turbo** — the speech model. Whisper is by OpenAI. The GGUF
build used by Draft is distributed by handy-computer. The model is distributed
with Draft (or fetched once when building from source) and is subject to its own
terms; see the model page.
[Model](https://huggingface.co/handy-computer/whisper-large-v3-turbo-gguf) ·
[Whisper](https://github.com/openai/whisper)

## Type

**Inter** — the interface typeface, designed by Rasmus Andersson. Licensed
under the SIL Open Font License 1.1.
[Source](https://github.com/rsms/inter)

**JetBrains Mono** — the live timer and command-line typeface, by JetBrains.
Licensed under the SIL Open Font License 1.1.
[Source](https://github.com/JetBrains/JetBrainsMono)

The font files and their license texts are bundled in `assets/fonts` and are
installed beside the app in `licenses`.

## Rust and platform crates

Draft is built with the Rust standard library and a focused set of crates for
audio capture, clipboard insertion, the Windows tray, logging, and model
downloads. The complete dependency list and the license reported by each crate
are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). The full text of every
license is distributed with the corresponding crate.

## Icons and brand

The Draft signal mark, the application icon, the tray icon, and the installer
icon are original work drawn from the same geometry. No third-party icon set is
used.

## Privacy

Draft has no telemetry, analytics, crash reporting, or update pings. The only
runtime download is the speech model, which is checksum-verified. The GitHub and
X actions on the Credits page open your browser when you choose them; Draft
itself makes no network request for them.

## License

Draft is released under the [MIT License](LICENSE). The bundled typefaces are
under the SIL Open Font License 1.1. The speech model is distributed with Draft
and is subject to its own terms.
