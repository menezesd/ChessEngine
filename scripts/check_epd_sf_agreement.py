#!/usr/bin/env python3
"""Check how often Stockfish's best legal child agrees with EPD bm."""

import argparse
import queue
import re
import subprocess
import threading
import time

import chess


class Stockfish:
    def __init__(self, path: str, depth: int):
        self.depth = depth
        self.proc = subprocess.Popen(
            [path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )
        self.lines = queue.Queue()
        threading.Thread(target=self._reader, daemon=True).start()
        self.send("uci")
        self.wait("uciok")
        self.send("setoption name Threads value 1")
        self.send("setoption name Hash value 128")
        self.send("isready")
        self.wait("readyok")

    def _reader(self):
        for line in self.proc.stdout:
            self.lines.put(line.strip())

    def send(self, cmd: str):
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def wait(self, token: str, timeout: float = 10.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if line.startswith(token):
                return line
        raise RuntimeError(f"timeout waiting for {token}")

    def eval(self, fen: str) -> int | None:
        self.send(f"position fen {fen}")
        self.send(f"go depth {self.depth}")
        score = None
        deadline = time.time() + 60.0
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if match := re.search(r"score cp (-?\d+)", line):
                score = int(match.group(1))
            elif match := re.search(r"score mate (-?\d+)", line):
                mate = int(match.group(1))
                score = 30000 - abs(mate) * 100
                if mate < 0:
                    score = -score
            if line.startswith("bestmove"):
                return score
        return score

    def quit(self):
        self.send("quit")
        try:
            self.proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.proc.kill()


def parse_epd(line: str):
    board = chess.Board()
    ops = board.set_epd(line.strip())
    return board, set(ops.get("bm", [])), ops.get("id", "unknown")


def child_score_for_parent(board: chess.Board, sf: Stockfish, move: chess.Move) -> int | None:
    child = board.copy(stack=False)
    child.push(move)
    score_for_child_stm = sf.eval(child.fen())
    if score_for_child_stm is None:
        return None
    # Convert child STM score to parent STM score after making move.
    return -score_for_child_stm


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--epd", required=True)
    parser.add_argument("--sf-path", default="/opt/local/bin/stockfish")
    parser.add_argument("--depth", type=int, default=10)
    parser.add_argument("--limit", type=int, default=0)
    args = parser.parse_args()

    sf = Stockfish(args.sf_path, args.depth)
    total = agree = 0
    try:
        with open(args.epd) as f:
            for line in f:
                if not line.strip():
                    continue
                if args.limit and total >= args.limit:
                    break
                board, best_moves, epd_id = parse_epd(line)
                scored = []
                for move in board.legal_moves:
                    score = child_score_for_parent(board, sf, move)
                    if score is not None:
                        scored.append((score, move))
                if not scored:
                    continue
                scored.sort(reverse=True, key=lambda item: item[0])
                top_score, top_move = scored[0]
                expected = max(
                    ((score, move) for score, move in scored if move in best_moves),
                    default=None,
                    key=lambda item: item[0],
                )
                total += 1
                ok = top_move in best_moves
                agree += int(ok)
                if not ok and total <= 30:
                    exp_text = "none" if expected is None else f"{board.san(expected[1])}:{expected[0]}"
                    print(f"MISS {epd_id}: sf={board.san(top_move)}:{top_score} epd={exp_text}")
    finally:
        sf.quit()
    print(f"Agreement: {agree}/{total}")


if __name__ == "__main__":
    main()
