#!/usr/bin/env python3
"""Sample FEN/result rows from PGN files for eval tuning."""

from __future__ import annotations

import argparse
import random
from pathlib import Path

import chess.pgn


def result_value(result: str) -> float | None:
    if result == "1-0":
        return 1.0
    if result == "0-1":
        return 0.0
    if result == "1/2-1/2":
        return 0.5
    return None


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("pgn", nargs="+", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--max-positions", type=int, default=50_000)
    parser.add_argument("--max-games", type=int, default=0)
    parser.add_argument("--skip-plies", type=int, default=12)
    parser.add_argument("--stride", type=int, default=6)
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    rng = random.Random(args.seed)
    rows: list[tuple[str, float]] = []
    games = 0

    for path in args.pgn:
        with path.open(errors="replace") as handle:
            while True:
                game = chess.pgn.read_game(handle)
                if game is None:
                    break
                games += 1
                result = result_value(game.headers.get("Result", "*"))
                if result is None:
                    continue

                board = game.board()
                for ply, move in enumerate(game.mainline_moves(), start=1):
                    board.push(move)
                    if ply <= args.skip_plies or ply % args.stride != 0:
                        continue
                    if board.is_game_over(claim_draw=True):
                        continue
                    rows.append((board.fen(), result))
                    if len(rows) > args.max_positions:
                        rows[rng.randrange(len(rows))] = rows[-1]
                        rows.pop()

                if args.max_games and games >= args.max_games:
                    break
        if args.max_games and games >= args.max_games:
            break

    rng.shuffle(rows)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w") as out:
        for fen, result in rows:
            out.write(f"{fen} | {result:.1f}\n")
    print(f"sampled {len(rows):,} positions from {games:,} games -> {args.output}")


if __name__ == "__main__":
    main()
