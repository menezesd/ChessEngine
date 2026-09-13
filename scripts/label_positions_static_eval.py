#!/usr/bin/env python3
"""Label positions with the engine's static eval command."""

import argparse
import re
import time

import chess

if __package__:
    from .uci_client import UCIClient
else:
    from uci_client import UCIClient


EVAL_RE = re.compile(r"\bblended (-?\d+)")


class UCIEngine(UCIClient):
    def static_eval(self, fen: str) -> int | None:
        self.send(f"position fen {fen}")
        self.send("eval")
        # Consume the complete command response before labeling another FEN.
        self.send("isready")
        deadline = time.monotonic() + 5.0
        score = None
        try:
            while True:
                line = self.read_line(deadline)
                if match := EVAL_RE.search(line):
                    score = int(match.group(1))
                if line == "readyok":
                    return score
        except TimeoutError:
            self.quit()
            raise


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
