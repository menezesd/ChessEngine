#!/usr/bin/env python3
"""Quick WAC test with NNUE option."""

import argparse
import queue
import subprocess
import sys
import time
from pathlib import Path

import chess


class UCIEngine:
    def __init__(self, path: str, nnue_file: str = None, options: list[str] = None):
        self.proc = subprocess.Popen(
            [path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        self.lines = queue.Queue()
        self._start_reader()
        self._handshake(nnue_file, options or [])

    def _start_reader(self):
        import threading
        def loop():
            for line in self.proc.stdout:
                self.lines.put(line.rstrip("\n"))
        t = threading.Thread(target=loop, daemon=True)
        t.start()

    def _send(self, cmd):
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def _drain_until(self, token, timeout=5.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
                if token in line:
                    return
            except queue.Empty:
                continue
        raise RuntimeError(f"Timeout waiting for {token}")

    def _handshake(self, nnue_file, options):
        self._send("uci")
        self._drain_until("uciok")
        if nnue_file:
            self._send(f"setoption name EvalFile value {nnue_file}")
            self._send("setoption name UseNNUE value true")
        for option in options:
            self._send(f"setoption name {option}")
        self._send("isready")
        self._drain_until("readyok")

    def bestmove(self, fen, movetime_ms, depth=0):
        self._send("ucinewgame")
        self._send(f"position fen {fen}")
        if depth > 0:
            self._send(f"go depth {depth}")
        else:
            self._send(f"go movetime {movetime_ms}")

        deadline = time.time() + (60 if depth > 0 else movetime_ms / 1000 + 10)
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
                if line.startswith("bestmove"):
                    return line.split()[1]
            except queue.Empty:
                continue
        raise RuntimeError("Timeout waiting for bestmove")

    def quit(self):
        if self.proc.poll() is None:
            self._send("quit")
            try:
                self.proc.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def parse_epd(line):
    tokens = line.strip().split()
    if "bm" not in tokens:
        return None, None, None
    fen = " ".join(tokens[:4] + ["0", "1"])
    bm_idx = tokens.index("bm")
    bms = []
    for mv in tokens[bm_idx + 1:]:
        mv = mv.rstrip(";")
        if mv == "id":
            break
        bms.append(mv)
    epd_id = "unknown"
    if "id" in tokens:
        epd_id = tokens[tokens.index("id") + 1].strip('";')
    return fen, bms, epd_id


def uci_to_san(fen, uci):
    try:
        board = chess.Board(fen)
        move = chess.Move.from_uci(uci)
        return board.san(move)
    except:
        return uci


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--epd", default="tests/data/wac_all.epd")
    parser.add_argument("--limit", type=int, default=50)
    parser.add_argument("--movetime-ms", type=int, default=500)
    parser.add_argument("--depth", type=int, default=0)
    parser.add_argument("--nnue", default=None, help="Path to NNUE file")
    parser.add_argument(
        "--setoption",
        action="append",
        default=[],
        help="UCI option in the form 'Name value X'; may be repeated",
    )
    parser.add_argument("--print-misses", action="store_true")
    args = parser.parse_args()

    engine = UCIEngine(args.engine, args.nnue, args.setoption)
    total = 0
    correct = 0

    with open(args.epd) as f:
        for line in f:
            if not line.strip() or total >= args.limit:
                break
            fen, bms, epd_id = parse_epd(line)
            if fen is None:
                continue
            total += 1
            best = engine.bestmove(fen, args.movetime_ms, args.depth)
            san = uci_to_san(fen, best)
            if san in bms or best in bms:
                correct += 1
            elif args.print_misses:
                print(f"MISS {epd_id}: best={best}/{san} expected={','.join(bms)}")

    engine.quit()
    use_nnue = args.nnue is not None or any(
        option.lower().replace(" ", "") == "usennuevaluetrue"
        for option in args.setoption
    )
    mode = "NNUE" if use_nnue else "HCE"
    print(f"WAC ({mode}): {correct}/{total}")


if __name__ == "__main__":
    main()
