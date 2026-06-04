#!/usr/bin/env python3
"""Sweep HCE/NNUE blend values on the WAC suite."""

import argparse
import re
import subprocess
import sys


SCORE_RE = re.compile(r"WAC \([^)]+\): (\d+)/(\d+)")


def run_wac(
    engine: str,
    epd: str,
    limit: int,
    movetime_ms: int,
    depth: int,
    blend: int,
    nnue: str | None,
) -> tuple[int, int]:
    cmd = [
        sys.executable,
        "scripts/wac_eval_nnue.py",
        "--engine",
        engine,
        "--epd",
        epd,
        "--limit",
        str(limit),
        "--movetime-ms",
        str(movetime_ms),
        "--setoption",
        "UseNNUE value true",
        "--setoption",
        f"NnueHceBlend value {blend}",
    ]
    if nnue is not None:
        cmd.extend(["--nnue", nnue])
    if depth > 0:
        cmd.extend(["--depth", str(depth)])

    result = subprocess.run(cmd, text=True, capture_output=True)
    if result.returncode != 0:
        raise RuntimeError(
            f"WAC failed for blend={blend} with exit code {result.returncode}\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
    match = SCORE_RE.search(result.stdout)
    if not match:
        raise RuntimeError(f"Could not parse WAC output:\n{result.stdout}\n{result.stderr}")
    return int(match.group(1)), int(match.group(2))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--epd", default="tests/data/wac_all.epd")
    parser.add_argument("--limit", type=int, default=201)
    parser.add_argument("--movetime-ms", type=int, default=500)
    parser.add_argument("--depth", type=int, default=0)
    parser.add_argument("--nnue", default=None)
    parser.add_argument(
        "--blends",
        type=int,
        nargs="+",
        default=[0, 25, 50, 75, 90, 100],
        help="NNUE percentages. 0 is HCE, 100 is pure NNUE.",
    )
    args = parser.parse_args()

    best = None
    for blend in args.blends:
        try:
            correct, total = run_wac(
                args.engine,
                args.epd,
                args.limit,
                args.movetime_ms,
                args.depth,
                blend,
                args.nnue,
            )
        except RuntimeError as err:
            print(err, flush=True)
            continue
        print(f"blend={blend:3d} score={correct}/{total}", flush=True)
        if best is None or correct > best[1]:
            best = (blend, correct, total)

    if best is not None:
        print(f"best blend={best[0]} score={best[1]}/{best[2]}")


if __name__ == "__main__":
    main()
