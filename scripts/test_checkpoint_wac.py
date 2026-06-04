#!/usr/bin/env python3
"""Test a training checkpoint on WAC by exporting to NNUE and running WAC test.

Usage:
    python3 scripts/test_checkpoint_wac.py checkpoint_improved_epoch_5.pt
    python3 scripts/test_checkpoint_wac.py runs/strong_nnue.nnue  # also accepts .nnue files directly
"""
import argparse
import os
import queue
import subprocess
import sys
import tempfile
import threading
import time

import chess
import torch

sys.path.insert(0, "scripts")
from train_nnue_improved import NNUE256


def export_checkpoint_to_nnue(checkpoint_path: str) -> str:
    """Export a .pt checkpoint to a temporary .nnue file."""
    device = torch.device("cpu")
    model = NNUE256().to(device)
    ckpt = torch.load(checkpoint_path, map_location=device)
    model.load_state_dict(ckpt["model_state_dict"])
    model.eval()

    tmp = tempfile.NamedTemporaryFile(suffix=".nnue", delete=False)
    model.export_quantized(tmp.name)
    return tmp.name


class UCIEngine:
    def __init__(self, path, nnue_file=None):
        self.proc = subprocess.Popen(
            [path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        self.lines = queue.Queue()
        threading.Thread(
            target=lambda: [self.lines.put(l.rstrip("\n")) for l in self.proc.stdout],
            daemon=True,
        ).start()
        self._send("uci")
        self._wait("uciok")
        if nnue_file:
            self._send(f"setoption name EvalFile value {nnue_file}")
            self._send("setoption name UseNNUE value true")
        self._send("isready")
        self._wait("readyok")

    def _send(self, cmd):
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def _wait(self, token, timeout=5.0):
        dl = time.time() + timeout
        while time.time() < dl:
            try:
                line = self.lines.get(timeout=0.1)
                if token in line:
                    return
            except queue.Empty:
                continue

    def bestmove(self, fen, movetime_ms):
        self._send("ucinewgame")
        self._send(f"position fen {fen}")
        self._send(f"go movetime {movetime_ms}")
        dl = time.time() + movetime_ms / 1000 + 10
        while time.time() < dl:
            try:
                line = self.lines.get(timeout=0.1)
                if line.startswith("bestmove"):
                    return line.split()[1]
            except queue.Empty:
                continue

    def quit(self):
        self._send("quit")
        try:
            self.proc.wait(timeout=2)
        except:
            self.proc.kill()


def parse_epd(line):
    tokens = line.strip().split()
    if "bm" not in tokens:
        return None, None
    fen = " ".join(tokens[:4] + ["0", "1"])
    bm_idx = tokens.index("bm")
    bms = []
    for mv in tokens[bm_idx + 1 :]:
        mv = mv.rstrip(";")
        if mv == "id":
            break
        bms.append(mv)
    return fen, bms


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("checkpoint", help=".pt checkpoint or .nnue file")
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--epd", default="tests/data/wac_all.epd")
    parser.add_argument("--limit", type=int, default=201)
    parser.add_argument("--movetime-ms", type=int, default=1000)
    args = parser.parse_args()

    # Export checkpoint to NNUE if needed
    if args.checkpoint.endswith(".pt"):
        print(f"Exporting {args.checkpoint} to NNUE...", flush=True)
        nnue_path = export_checkpoint_to_nnue(args.checkpoint)
        cleanup = True
    else:
        nnue_path = args.checkpoint
        cleanup = False

    try:
        engine = UCIEngine(args.engine, nnue_path)
        total = correct = 0
        with open(args.epd) as f:
            for line in f:
                if not line.strip() or total >= args.limit:
                    break
                fen, bms = parse_epd(line)
                if fen is None:
                    continue
                total += 1
                best = engine.bestmove(fen, args.movetime_ms)
                try:
                    board = chess.Board(fen)
                    san = board.san(chess.Move.from_uci(best))
                except:
                    san = best
                if san in bms or best in bms:
                    correct += 1
                if total % 50 == 0:
                    print(f"  Progress: {correct}/{total}", flush=True)
        engine.quit()
        print(f"\nWAC: {correct}/{total} ({100*correct/total:.1f}%)")
    finally:
        if cleanup:
            os.unlink(nnue_path)


if __name__ == "__main__":
    main()
