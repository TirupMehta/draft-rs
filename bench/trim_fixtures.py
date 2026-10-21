"""Trim the generated TTS fixtures to exact benchmark durations.

The Windows voice speaks at its own rate, so the raw files are longer than their
names. Trimming keeps the baseline and the Rust engine on identical clips.
"""

from __future__ import annotations

import wave
from pathlib import Path

TARGETS = {
    "tts-3s.wav": 3.0,
    "tts-5s.wav": 5.0,
    "tts-15s.wav": 15.0,
    "tts-30s.wav": 30.0,
    "tts-60s.wav": 60.0,
}


def main() -> None:
    root = Path(__file__).resolve().parent / "fixtures"
    for name, seconds in TARGETS.items():
        path = root / name
        if not path.exists():
            raise SystemExit(f"missing {path}; add the fixture first")
        with wave.open(str(path), "rb") as source:
            params = source.getparams()
            rate = source.getframerate()
            frames = source.readframes(int(rate * seconds))
        with wave.open(str(path), "wb") as target:
            target.setparams(params)
            target.writeframes(frames)
        print(f"{name}: {seconds:.0f}s")


if __name__ == "__main__":
    main()
