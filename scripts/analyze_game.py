#!/usr/bin/env python3
"""
Analyze a single game (PGN) with a UCI engine and emit per-move evaluations.

Examples
--------
  # Analyze with this engine at depth 12
  python3 scripts/analyze_game.py --pgn game.pgn --engine ./target/debug/chess_engine --depth 12

  # Analyze with Stockfish for a quick sanity check
  python3 scripts/analyze_game.py --pgn game.pgn --engine /usr/games/stockfish --movetime 200
"""

import argparse
import sys
import chess.pgn
import subprocess
import sys
import time
from typing import Optional


def parse_score(info_line: str) -> Optional[str]:
    parts = info_line.split()
    if "score" not in parts:
        return None
    try:
        idx = parts.index("score")
        kind = parts[idx + 1]
        val = parts[idx + 2]
        if kind == "cp":
            return f"cp {val}"
        if kind == "mate":
            return f"mate {val}"
    except Exception:
        return None
    return None


class UCIEngine:
    def __init__(self, cmd):
        self.cmd = cmd
        self.proc = None

    def start(self):
        self.proc = subprocess.Popen(
            self.cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            universal_newlines=True,
        )
        self._send("uci")
        self._drain_until("uciok")
        self._send("isready")
        self._drain_until("readyok")

    def _send(self, line: str):
        assert self.proc and self.proc.stdin
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()

    def _drain_until(self, token: str, timeout: float = 5.0):
        assert self.proc and self.proc.stdout
        deadline = time.time() + timeout
        while True:
            if time.time() > deadline:
                raise RuntimeError(f"timeout waiting for {token}")
            line = self.proc.stdout.readline()
            if not line:
                time.sleep(0.01)
                continue
            line = line.strip()
            if line.startswith(token):
                return line

    def analyze_fen(self, fen: str, movetime_ms: int) -> str:
        assert self.proc and self.proc.stdout
        self._send(f"position fen {fen}")
        self._send(f"go movetime {movetime_ms}")
        last_score = None
        while True:
            line = self.proc.stdout.readline()
            if not line:
                time.sleep(0.01)
                continue
            line = line.strip()
            if line.startswith("info"):
                sc = parse_score(line)
                if sc:
                    last_score = sc
            if line.startswith("bestmove"):
                return last_score or "unknown"

    def quit(self):
        if not self.proc:
            return
        try:
            self._send("quit")
        except Exception:
            pass
        self.proc.terminate()
        self.proc = None


def analyze_game(engine_path: str, pgn_path: str, depth: int, movetime: int, out_path: str | None):
    with open(pgn_path, "r", encoding="utf-8") as f:
        game = chess.pgn.read_game(f)

    if game is None:
        raise RuntimeError(f"No game found in {pgn_path}")

    engine = UCIEngine([engine_path])
    engine.start()

    board = game.board()
    node = game
    ply = 0

    print(f"Analyzing {pgn_path} with {engine_path} (depth={depth}, movetime={movetime} ms)")
    while node.variations:
        node = node.variation(0)
        move = node.move
        board.push(move)
        fen = board.fen()
        comment = engine.analyze_fen(fen, movetime if movetime > 0 else 200)
        node.comment = comment
        ply += 1
        move_number = ply // 2 + (1 if ply % 2 else 0)
        prefix = f"{move_number}..." if ply % 2 == 0 else f"{move_number}."
        print(f"{prefix} {board.peek()} -> {comment}")

    engine.quit()

    if out_path:
        with open(out_path, "w", encoding="utf-8") as f:
            print(game, file=f, end="\n\n")
        print(f"\nAnnotated PGN written to {out_path}")


def main():
    parser = argparse.ArgumentParser(description="Analyze a PGN game with a UCI engine.")
    parser.add_argument("--pgn", required=True, help="Path to PGN file containing a single game")
    parser.add_argument("--engine", default="./target/debug/chess_engine", help="Path to UCI engine binary")
    parser.add_argument("--depth", type=int, default=10, help="Search depth (ignored if --movetime set)")
    parser.add_argument("--movetime", type=int, default=0, help="Per-position movetime in ms (overrides depth if >0)")
    parser.add_argument("--out", help="Optional path to write annotated PGN")
    args = parser.parse_args()

    try:
        analyze_game(args.engine, args.pgn, args.depth, args.movetime, args.out)
    except Exception as exc:  # pragma: no cover - CLI helper
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
