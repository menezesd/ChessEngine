#!/usr/bin/env python3
"""Label positions with the engine's static eval command."""

import argparse
import queue
import re
import subprocess
import threading
import time

import chess


EVAL_RE = re.compile(r"\bblended (-?\d+)")


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

    def static_eval(self, fen: str) -> int | None:
        self.send(f"position fen {fen}")
        self.send("eval")
        deadline = time.time() + 5.0
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if match := EVAL_RE.search(line):
                return int(match.group(1))
        return None

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
    for path in paths:
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    if "|" in line:
                        fen = line.split("|", 1)[0].strip()
                        board = chess.Board(fen)
                    else:
                        try:
                            board = chess.Board(line)
                            fen = board.fen()
                        except Exception:
                            board = chess.Board()
                            board.set_epd(line)
                            fen = board.fen()
                except Exception:
                    continue
                if fen in seen:
                    continue
                seen.add(fen)
                positions.append(fen)
                if limit and len(positions) >= limit:
                    return positions
    return positions


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", nargs="+", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--engine", default="./target/release/chess_engine")
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
                eval_stm = engine.static_eval(fen)
                if eval_stm is None:
                    continue
                board = chess.Board(fen)
                white_eval = eval_stm if board.turn == chess.WHITE else -eval_stm
                out.write(f"{fen} | {white_eval} | {result_from_white_eval(white_eval):.1f}\n")
                written += 1
                if i % 5000 == 0:
                    print(f"  labeled {i:,}/{len(positions):,}", flush=True)
    finally:
        engine.quit()
    print(f"Wrote {written:,} rows to {args.output}")


if __name__ == "__main__":
    main()
