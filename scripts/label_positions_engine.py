#!/usr/bin/env python3
"""Label FEN/EPD positions with this engine's UCI search score."""

import argparse
import queue
import re
import subprocess
import threading
import time

import chess


SCORE_CP_RE = re.compile(r"\bscore cp (-?\d+)")
SCORE_MATE_RE = re.compile(r"\bscore mate (-?\d+)")


class UCIEngine:
    def __init__(self, path: str, options: list[str]):
        self.proc = subprocess.Popen(
            [path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        self.lines: queue.Queue[str] = queue.Queue()
        threading.Thread(target=self._reader, daemon=True).start()
        self.send("uci")
        self.wait_for("uciok")
        for option in options:
            self.send(f"setoption name {option}")
        self.send("isready")
        self.wait_for("readyok")

    def _reader(self):
        for line in self.proc.stdout:
            self.lines.put(line.rstrip("\n"))

    def send(self, cmd: str):
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def wait_for(self, token: str, timeout: float = 10.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if token in line:
                return line
        raise RuntimeError(f"timeout waiting for {token}")

    def score(self, fen: str, depth: int, movetime_ms: int) -> int | None:
        self.send("ucinewgame")
        self.send(f"position fen {fen}")
        if depth > 0:
            self.send(f"go depth {depth}")
            timeout = 60.0
        else:
            self.send(f"go movetime {movetime_ms}")
            timeout = movetime_ms / 1000 + 10

        score = None
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if match := SCORE_CP_RE.search(line):
                score = int(match.group(1))
            elif match := SCORE_MATE_RE.search(line):
                mate = int(match.group(1))
                score = 30000 - abs(mate) * 100
                if mate < 0:
                    score = -score
            if line.startswith("bestmove"):
                return score
        return score

    def quit(self):
        if self.proc.poll() is None:
            self.send("quit")
            try:
                self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def result_from_white_eval(eval_cp: int) -> float:
    if eval_cp > 100:
        return 1.0
    if eval_cp < -100:
        return 0.0
    return 0.5


def read_positions(paths: list[str], limit: int) -> list[str]:
    positions = []
    seen = set()

    def add_fen(fen: str) -> bool:
        nonlocal positions
        try:
            board = chess.Board(fen)
        except Exception:
            return False
        canonical = board.fen()
        if canonical in seen:
            return False
        seen.add(canonical)
        positions.append(canonical)
        return bool(limit and len(positions) >= limit)

    for path in paths:
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                if "|" in line:
                    for part in line.split("|")[:2]:
                        if add_fen(part.strip()):
                            return positions
                    continue
                try:
                    board = chess.Board(line)
                    if add_fen(board.fen()):
                        return positions
                    continue
                except Exception:
                    pass
                try:
                    board = chess.Board()
                    board.set_epd(line)
                except Exception:
                    continue
                if add_fen(board.fen()):
                    return positions
    return positions


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", nargs="+", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--depth", type=int, default=4)
    parser.add_argument("--movetime-ms", type=int, default=0)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--setoption", action="append", default=[])
    args = parser.parse_args()

    positions = read_positions(args.input, args.limit)
    print(f"Labeling {len(positions):,} positions")
    engine = UCIEngine(args.engine, args.setoption)
    written = 0
    try:
        with open(args.output, "w") as out:
            for i, fen in enumerate(positions, 1):
                score = engine.score(fen, args.depth, args.movetime_ms)
                if score is None:
                    continue
                board = chess.Board(fen)
                white_eval = score if board.turn == chess.WHITE else -score
                out.write(f"{fen} | {white_eval} | {result_from_white_eval(white_eval):.1f}\n")
                written += 1
                if i % 1000 == 0:
                    print(f"  labeled {i:,}/{len(positions):,}", flush=True)
    finally:
        engine.quit()
    print(f"Wrote {written:,} rows to {args.output}")


if __name__ == "__main__":
    main()
