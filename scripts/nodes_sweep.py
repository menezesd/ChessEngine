#!/usr/bin/env python3
"""
Run node-limited searches across pruning settings and compare best moves.
"""

from __future__ import annotations

import argparse
import os
import subprocess


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/release/chess_engine", help="Path to UCI engine")
    parser.add_argument("--fen", required=True, help="FEN (6 fields)")
    parser.add_argument("--nodes", type=int, default=200000, help="Node limit")
    parser.add_argument("--variants", default="baseline,conservative", help="Comma list of SEARCH_STYLE")
    return parser.parse_args()


def run_variant(engine: str, fen: str, nodes: int, style: str) -> str:
    proc = subprocess.Popen(
        [engine],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        env={**os.environ, "SEARCH_STYLE": style},
    )

    def send(cmd: str) -> None:
        assert proc.stdin is not None
        proc.stdin.write(cmd + "\n")
        proc.stdin.flush()

    def drain_until(token: str) -> None:
        assert proc.stdout is not None
        while True:
            line = proc.stdout.readline()
            if not line:
                continue
            if token in line:
                return

    send("uci")
    drain_until("uciok")
    send("isready")
    drain_until("readyok")
    send(f"position fen {fen}")
    send(f"go nodes {nodes}")

    best = None
    assert proc.stdout is not None
    while True:
        line = proc.stdout.readline()
        if not line:
            continue
        if line.startswith("bestmove "):
            best = line.split()[1].strip()
            break

    send("quit")
    proc.wait(timeout=2)
    return best or "(none)"


def main() -> int:
    args = parse_args()
    variants = [v.strip() for v in args.variants.split(",") if v.strip()]
    for style in variants:
        best = run_variant(args.engine, args.fen, args.nodes, style)
        print(f"{style}: bestmove {best}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
