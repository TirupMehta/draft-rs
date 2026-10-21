# Contributing to Draft

Draft is a small, focused Windows app. Contributions are welcome when they keep
it fast, private, and honest.

## Before you start

Read these first:

- [AGENTS.md](AGENTS.md) - the working agreement, design system, build rules,
  and the hard constraints.
- [docs/SPEC.md](docs/SPEC.md) - the product contract.
- [docs/ROADMAP.md](docs/ROADMAP.md) - what is actually planned.
- [docs/PERFORMANCE.md](docs/PERFORMANCE.md) - the speed budget and method.

Windows x64 is the supported target today. macOS and Linux are future work;
please do not add a platform shell just to make a change compile.

## Set up the toolchain

You need Rust stable, Microsoft C++ Build Tools, CMake, and the Vulkan SDK.
Install those, then:

```text
cargo run -p draft-desktop
```

The app looks for the F16 model at
`%LOCALAPPDATA%\Draft4\models\whisper-large-v3-turbo-F16.gguf`. Install or
verify it with:

```text
cargo run -p draft -- model install
cargo run -p draft -- model status
```

## Make a change

- Keep the change focused. A bug fix does not need a surrounding cleanup.
- Follow the design system in `apps/desktop/src/theme.rs`. Do not hard-code a
  color, size, radius, or font in a view.
- The interface must be truthful. Never fake a recording, a level, a model
  state, a latency number, or a permission.
- Keep idle at zero repaints. If you add motion, it must settle and stop.
- No new runtime dependency, telemetry, crash reporting, or network call
  without a clear reason in the pull request. The model install (or download,
  when building without the bundled model) is the one documented exception.
- Put test-only affordances behind `debug_assertions`, never in the shipped
  path.
- Update the docs when behavior, defaults, or the design rules change.

## Run the gates

These are the same checks CI runs:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

For a UI change, capture the rendered window and check the text positions, clipping,
and both the light and dark palettes.

## Pull requests

- One logical change per pull request.
- Describe the problem, the change, and how you verified it.
- Include before/after captures for a visual change.
- Keep commit messages one line in the imperative mood, for example
  `fix: keep the HUD inside the work area`. Put the reasoning in the body.
- Do not commit `target/`, `dist/`, `.commandcode/`, or local scratch files.

## License

Draft is released under the [MIT License](LICENSE). By submitting a contribution,
you agree that it is licensed under the same terms. There is no separate
contributor license agreement.

## Security

Please do not open a public issue for a security problem. Use GitHub's private
security advisory for this repository, or contact the maintainer through the
links in [CREDITS.md](CREDITS.md).

## Questions

Open a discussion or an issue on
[GitHub](https://github.com/TirupMehta/draft-rs). For the fastest answer,
include the Windows version, the GPU backend, and the relevant log lines from
`%LOCALAPPDATA%\Draft4\logs`.
