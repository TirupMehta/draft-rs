"""Measure the Draft v3.5 engine on the reference machine.

This is a benchmark tool, not part of the app. It imports the Python 3.13
packages bundled inside the shipping Draft v3.5 build, so the numbers come from
the exact faster-whisper / CTranslate2 / large-v3-turbo CUDA path users run.

Usage:
    python bench/baseline_v3.py --fixtures bench/fixtures --runs 10 --json bench/results/v3-baseline.json

Set DRAFT_V3_BUNDLE if the frozen build lives somewhere else.
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import sys
import time
import wave
from pathlib import Path

DEFAULT_BUNDLE = Path(r"C:\all\Draft\dist\Draft\_internal")


def prepare_bundle(bundle: Path) -> None:
    if not bundle.is_dir():
        raise SystemExit(f"bundle not found: {bundle}")
    ctranslate = bundle / "ctranslate2"
    os.environ["PATH"] = os.pathsep.join(
        [str(bundle), str(ctranslate), os.environ.get("PATH", "")]
    )
    os.add_dll_directory(str(bundle))
    os.add_dll_directory(str(ctranslate))
    sys.path.insert(0, str(bundle))


def read_wav(path: Path):
    import numpy as np

    with wave.open(str(path), "rb") as wav:
        if wav.getnchannels() != 1 or wav.getsampwidth() != 2:
            raise SystemExit(f"{path.name}: expected mono 16-bit PCM")
        rate = wav.getframerate()
        if rate != 16_000:
            raise SystemExit(f"{path.name}: expected 16 kHz, got {rate}")
        data = np.frombuffer(wav.readframes(wav.getnframes()), dtype=np.int16)
    return data.astype(np.float32) / 32768.0


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, int(round(fraction * (len(ordered) - 1)))))
    return ordered[index]


def transcribe(model, audio, beam_size: int, vad: bool) -> tuple[str, float]:
    started = time.perf_counter()
    segments, _info = model.transcribe(
        audio,
        language=None,
        beam_size=beam_size,
        vad_filter=vad,
        condition_on_previous_text=False,
        temperature=0.0,
    )
    text = " ".join(
        segment.text.strip()
        for segment in segments
        if segment.text.strip() and float(getattr(segment, "no_speech_prob", 0.0) or 0.0) <= 0.6
    )
    return text, (time.perf_counter() - started) * 1000.0


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--bundle",
        type=Path,
        default=Path(os.environ.get("DRAFT_V3_BUNDLE", DEFAULT_BUNDLE)),
    )
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=10)
    parser.add_argument("--json", type=Path, required=True)
    args = parser.parse_args()

    prepare_bundle(args.bundle)

    import numpy as np
    from faster_whisper import WhisperModel

    print("loading large-v3-turbo on cuda/float16 ...", flush=True)
    model = WhisperModel("large-v3-turbo", device="cuda", compute_type="float16")
    print("warmup ...", flush=True)
    transcribe(model, np.zeros(16_000, dtype=np.float32), beam_size=5, vad=True)

    results: dict[str, dict] = {}
    for path in sorted(args.fixtures.glob("*.wav")):
        audio = read_wav(path)
        duration = len(audio) / 16_000.0
        final_ms: list[float] = []
        text = ""
        for _ in range(args.runs):
            text, elapsed = transcribe(model, audio, beam_size=5, vad=True)
            final_ms.append(elapsed)
        results[path.name] = {
            "duration_s": round(duration, 3),
            "final_ms": {
                "p50": round(statistics.median(final_ms), 1),
                "p95": round(percentile(final_ms, 0.95), 1),
            },
            "rtf": round(duration / (statistics.median(final_ms) / 1000.0), 2),
            "text": text,
        }
        print(
            f"{path.name}: {duration:.1f}s  p50={results[path.name]['final_ms']['p50']}ms  "
            f"p95={results[path.name]['final_ms']['p95']}ms  rtf={results[path.name]['rtf']}x",
            flush=True,
        )

    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(json.dumps(results, indent=2), encoding="utf-8")
    print(f"wrote {args.json}")


if __name__ == "__main__":
    main()
