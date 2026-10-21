# Bundled fonts

These files are embedded in the Draft binary with `include_bytes!` and
registered at startup, so type renders identically on every Windows machine.

| File | Source | License |
|---|---|---|
| `Inter-Regular.ttf`, `Inter-Medium.ttf`, `Inter-SemiBold.ttf` | [Inter 4.1](https://github.com/rsms/inter/releases/tag/v4.1) | SIL OFL 1.1 (`Inter-OFL.txt`) |
| `JetBrainsMono-Regular.ttf`, `JetBrainsMono-Medium.ttf` | [JetBrains Mono 2.304](https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304) | SIL OFL 1.1 (`JetBrainsMono-OFL.txt`) |

Inter is the interface face. JetBrains Mono is used only for the live timer
and the command-line example, where tabular figures matter.
