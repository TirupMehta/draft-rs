# Bundled fonts

These files are embedded in the Draft binary with `include_bytes!` and
registered at startup, so type renders identically on every Windows machine.

| File | Source | License |
|---|---|---|
| `InstrumentSans-Regular.ttf`, `InstrumentSans-Medium.ttf`, `InstrumentSans-SemiBold.ttf` | [Instrument Sans](https://github.com/Instrument/instrument-sans), static instances of the variable font at weights 400, 500 and 600, Latin subset | SIL OFL 1.1 (`InstrumentSans-OFL.txt`) |
| `InstrumentSerif-Regular.ttf` | [Instrument Serif](https://github.com/Instrument/instrument-serif), Latin subset | SIL OFL 1.1 (`InstrumentSerif-OFL.txt`) |

Instrument Sans is the interface face. Instrument Serif carries the few large
lines (the hero line, the section titles, the live timer) and is only ever set
at its single regular weight. The same two faces set the website, so the app and
the site read as one product. The weights were instanced from the website's
variable font; nothing here is fetched at build or run time.
