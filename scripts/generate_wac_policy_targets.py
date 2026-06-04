#!/usr/bin/env python3
"""Generate WAC-derived child-position eval targets for NNUE fine-tuning."""

import argparse
import random

import chess


def parse_epd(line: str):
    board = chess.Board()
    operation = board.set_epd(line.strip())
    best_moves = operation.get("bm", [])
    epd_id = operation.get("id", "unknown")
    return board, set(best_moves), epd_id


def result_from_white_eval(eval_cp: int) -> float:
    if eval_cp > 100:
        return 1.0
    if eval_cp < -100:
        return 0.0
    return 0.5


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--epd", default="tests/data/wac_all.epd")
    parser.add_argument("--output", default="data/wac_policy_targets.txt")
    parser.add_argument("--best-cp", type=int, default=900)
    parser.add_argument("--other-cp", type=int, default=-150)
    parser.add_argument("--best-repeat", type=int, default=12)
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    rows = []
    with open(args.epd) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            board, best_moves, _epd_id = parse_epd(line)
            parent_white = board.turn == chess.WHITE
            for move in board.legal_moves:
                child = board.copy(stack=False)
                child.push(move)

                parent_cp = args.best_cp if move in best_moves else args.other_cp
                white_cp = parent_cp if parent_white else -parent_cp
                result = result_from_white_eval(white_cp)
                repeat = args.best_repeat if move in best_moves else 1
                row = f"{child.fen()} | {white_cp} | {result:.1f}"
                rows.extend([row] * repeat)

    random.Random(args.seed).shuffle(rows)
    with open(args.output, "w") as f:
        for row in rows:
            f.write(row + "\n")
    print(f"Wrote {len(rows):,} rows to {args.output}")


if __name__ == "__main__":
    main()
