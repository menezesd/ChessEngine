#!/usr/bin/env python3
"""Collect ranking pairs from EPD positions missed by a UCI engine."""

import argparse
import queue
import subprocess
import threading
import time

import chess


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

    def bestmove(self, fen: str, movetime_ms: int) -> chess.Move | None:
        self.send("ucinewgame")
        self.send(f"position fen {fen}")
        self.send(f"go movetime {movetime_ms}")
        deadline = time.time() + movetime_ms / 1000 + 10
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if line.startswith("bestmove"):
                parts = line.split()
                if len(parts) < 2 or parts[1] == "(none)":
                    return None
                return chess.Move.from_uci(parts[1])
        return None

    def quit(self):
        if self.proc.poll() is None:
            self.send("quit")
            try:
                self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def parse_epd(line: str):
    board = chess.Board()
    ops = board.set_epd(line.strip())
    return board, set(ops.get("bm", [])), ops.get("id", "unknown")


def child_fen(board: chess.Board, move: chess.Move) -> str:
    child = board.copy(stack=False)
    child.push(move)
    return child.fen()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--epd", nargs="+", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--movetime-ms", type=int, default=300)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--setoption", action="append", default=[])
    args = parser.parse_args()

    engine = UCIEngine(args.engine, args.setoption)
    total = misses = written = 0
    try:
        with open(args.output, "w") as out:
            for path in args.epd:
                with open(path) as f:
                    for line in f:
                        if not line.strip():
                            continue
                        if args.limit and total >= args.limit:
                            break
                        try:
                            board, best_moves, _epd_id = parse_epd(line)
                        except Exception:
                            continue
                        total += 1
                        picked = engine.bestmove(board.fen(), args.movetime_ms)
                        if picked is None or picked in best_moves:
                            continue
                        misses += 1
                        expected = next(iter(best_moves), None)
                        if expected is None:
                            continue
                        out.write(f"{child_fen(board, expected)} | {child_fen(board, picked)}\n")
                        written += 1
                if args.limit and total >= args.limit:
                    break
    finally:
        engine.quit()
    print(f"Checked {total:,}, misses {misses:,}, wrote {written:,} pairs to {args.output}")


if __name__ == "__main__":
    main()
