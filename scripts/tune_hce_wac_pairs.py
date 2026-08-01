#!/usr/bin/env python3
"""Fit coarse HCE multipliers from WAC expected-vs-picked child pairs."""

from __future__ import annotations

import argparse
import queue
import re
import subprocess
import threading
import time
from pathlib import Path

import chess
import torch
import torch.nn.functional as F

from wac_eval_nnue import parse_epd, uci_to_san


FEATURE_RE = re.compile(
    r"evalfeatures names (?P<names>\S+) values (?P<values>[-0-9,]+) "
    r"tempo (?P<tempo>-?\d+) phase (?P<phase>-?\d+) full_white (?P<full>-?\d+)"
)

CURRENT_WEIGHTS = torch.tensor(
    [
        1.078,
        1.052,
        1.091,
        0.978,
        1.052,
        1.046,
        1.069,
        1.001,
        1.002,
        1.094,
        1.134,
        0.972,
        1.019,
        0.967,
        1.035,
        1.009,
        1.029,
        0.975,
        1.004,
        1.005,
        1.005,
    ],
    dtype=torch.float32,
)


class Engine:
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

    def _reader(self) -> None:
        assert self.proc.stdout is not None
        for line in self.proc.stdout:
            self.lines.put(line.rstrip("\n"))

    def send(self, cmd: str) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def wait_for(self, token: str, timeout: float = 10.0) -> str:
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if token in line:
                return line
        raise RuntimeError(f"timeout waiting for {token}")

    def bestmove(self, fen: str, movetime_ms: int, depth: int) -> str:
        self.send("ucinewgame")
        self.send(f"position fen {fen}")
        if depth:
            self.send(f"go depth {depth}")
        else:
            self.send(f"go movetime {movetime_ms}")
        deadline = time.time() + (60 if depth else movetime_ms / 1000 + 10)
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if line.startswith("bestmove"):
                return line.split()[1]
        raise RuntimeError("timeout waiting for bestmove")

    def features(self, fen: str) -> tuple[list[str], list[int]]:
        self.send(f"position fen {fen}")
        self.send("evalfeatures")
        line = self.wait_for("evalfeatures")
        match = FEATURE_RE.search(line)
        if not match:
            raise RuntimeError(f"could not parse evalfeatures line: {line}")
        return match.group("names").split(","), [int(v) for v in match.group("values").split(",")]

    def quit(self) -> None:
        if self.proc.poll() is None:
            self.send("quit")
            try:
                self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def expected_moves(board: chess.Board, expected: list[str]) -> list[chess.Move]:
    moves: list[chess.Move] = []
    for token in expected:
        text = token.strip().rstrip(";")
        for candidate in (text, text.replace("!", "").replace("?", "")):
            try:
                moves.append(board.parse_san(candidate))
                break
            except ValueError:
                try:
                    move = chess.Move.from_uci(candidate)
                    if move in board.legal_moves:
                        moves.append(move)
                        break
                except ValueError:
                    pass
    return moves


def child_side_features(engine: Engine, board: chess.Board, move: chess.Move) -> tuple[list[str], torch.Tensor]:
    child = board.copy(stack=False)
    child.push(move)
    names, values = engine.features(child.fen())
    sign = 1.0 if child.turn == chess.WHITE else -1.0
    return names, torch.tensor(values, dtype=torch.float32) * sign


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--epd", default="tests/data/wac_all.epd")
    parser.add_argument("--limit", type=int, default=201)
    parser.add_argument("--movetime-ms", type=int, default=500)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--epochs", type=int, default=4000)
    parser.add_argument("--lr", type=float, default=0.01)
    parser.add_argument("--l2", type=float, default=0.4)
    parser.add_argument("--scale", type=float, default=400.0)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    options = [
        "UseNNUE value false",
        "UseFullHCE value true",
        "UseTunedHCE value true",
    ]
    engine = Engine(args.engine, options)
    diffs: list[torch.Tensor] = []
    names: list[str] | None = None
    total = correct = 0

    try:
        with open(args.epd) as f:
            for line in f:
                if not line.strip() or total >= args.limit:
                    break
                fen, expected, epd_id = parse_epd(line)
                if fen is None:
                    continue
                board = chess.Board(fen)
                expected_legal = expected_moves(board, expected)
                if not expected_legal:
                    continue

                total += 1
                picked_uci = engine.bestmove(fen, args.movetime_ms, args.depth)
                picked = chess.Move.from_uci(picked_uci)
                picked_san = uci_to_san(fen, picked_uci)
                if picked in expected_legal or picked_san in expected:
                    correct += 1
                    continue

                picked_names, picked_features = child_side_features(engine, board, picked)
                if names is None:
                    names = picked_names
                for expected_move in expected_legal:
                    expected_names, expected_features = child_side_features(engine, board, expected_move)
                    if expected_names != names:
                        raise RuntimeError("feature name mismatch")
                    # Search prefers lower child side-to-move eval. Make picked child score
                    # higher than the expected child under the tuned static evaluator.
                    diffs.append((picked_features - expected_features) / args.scale)
                print(
                    f"pair {epd_id}: picked={picked_uci}/{picked_san} "
                    f"expected={','.join(expected)}",
                    flush=True,
                )
    finally:
        engine.quit()

    if names is None or not diffs:
        raise RuntimeError("no WAC miss pairs collected")

    x = torch.stack(diffs)
    weights = torch.nn.Parameter(CURRENT_WEIGHTS.clone())
    opt = torch.optim.AdamW([weights], lr=args.lr, weight_decay=0.0)

    for epoch in range(args.epochs):
        opt.zero_grad()
        logits = x @ weights
        loss = F.binary_cross_entropy_with_logits(logits, torch.ones_like(logits))
        loss = loss + args.l2 * F.mse_loss(weights, CURRENT_WEIGHTS)
        loss.backward()
        opt.step()
        if (epoch + 1) % 1000 == 0:
            print(f"epoch {epoch + 1}: loss={loss.item():.6f}", flush=True)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w") as out:
        out.write("name,weight\n")
        for name, weight in zip(names, weights.detach().tolist()):
            out.write(f"{name},{weight:.6f}\n")

    print(f"baseline WAC: {correct}/{total}")
    print(f"fit {len(diffs)} WAC miss pairs -> {args.output}")
    moved = sorted(
        zip(names, weights.detach().tolist()),
        key=lambda item: abs(item[1] - CURRENT_WEIGHTS[names.index(item[0])].item()),
        reverse=True,
    )
    for name, weight in moved[:12]:
        print(f"  {name:18s} {weight:.3f}")


if __name__ == "__main__":
    main()
