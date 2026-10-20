"""Compare transcribe-bench results with the Draft v3.5 baseline.

The baseline stores final decode latency (p50/p95). A transcribe-bench result
stores one entry per measured iteration, so this script computes the same
statistics from `per_iter[].total_ms` and prints the speedup.

Usage:
    python bench/compare_bakeoff.py --results-dir bench/results --prefix vulkan-
"""

from __future__ import annotations

import argparse
import json
import statistics
from pathlib import Path


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, int(round(fraction * (len(ordered) - 1)))))
    return ordered[index]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, default=Path("bench/baseline.json"))
    parser.add_argument("--results-dir", type=Path, default=Path("bench/results"))
    parser.add_argument("--prefix", default="vulkan-")
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--gate", action="store_true", help="exit 1 if any p95 is slower")
    args = parser.parse_args()

    baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
    summary: dict[str, dict] = {}
    failed = False

    print(
        f"{'clip':<14} {'v3.5 p50':>10} {'v3.5 p95':>10} "
        f"{'new p50':>10} {'new p95':>10} {'speedup p50':>12} {'speedup p95':>12}"
    )
    print("-" * 84)

    for name, reference in baseline["fixtures"].items():
        result_path = args.results_dir / f"{args.prefix}{name}.json"
        if not result_path.exists():
            continue
        data = json.loads(result_path.read_text(encoding="utf-8"))
        values = [float(item["total_ms"]) for item in data["per_iter"]]
        p50 = statistics.median(values)
        p95 = percentile(values, 0.95)
        ref50 = float(reference["final_ms"]["p50"])
        ref95 = float(reference["final_ms"]["p95"])
        summary[name] = {
            "duration_s": reference["duration_s"],
            "v35_final_ms": {"p50": ref50, "p95": ref95},
            "new_final_ms": {"p50": round(p50, 1), "p95": round(p95, 1)},
            "speedup": {"p50": round(ref50 / p50, 2), "p95": round(ref95 / p95, 2)},
            "backend": data.get("backend"),
            "iters": data.get("iters"),
        }
        if p95 > ref95:
            failed = True
        print(
            f"{name:<14} {ref50:>9.0f}ms {ref95:>9.0f}ms "
            f"{p50:>9.0f}ms {p95:>9.0f}ms {ref50 / p50:>11.2f}x {ref95 / p95:>11.2f}x"
        )

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(summary, indent=2), encoding="utf-8")
        print(f"\nwrote {args.json_out}")

    if args.gate and failed:
        raise SystemExit("gate failed: at least one p95 is slower than v3.5")


if __name__ == "__main__":
    main()
