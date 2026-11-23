#!/usr/bin/env python3
"""Head-to-head match between two NNUE files of the same engine binary.

Plays N games at fixed movetime:
  engine(netA) vs engine(netB) and the reverse colors, alternating openings.
Usage:
  python3 scripts/match_nnue.py --engine target/release/chess_engine \
      --net-a runs/exp1/improved.nnue --net-b src/board/nnue/nets/default.nnue \
      --games 20 --movetime 100 --plies 60
"""
from __future__ import annotations

import argparse
import pathlib
import random
import time

import chess
import chess.engine


def configured(path: str, net: str) -> chess.engine.SimpleEngine:
    engine = chess.engine.SimpleEngine.popen_uci(path)
    engine.configure({"UseNNUE": True, "EvalFile": net})
    return engine


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="target/release/chess_engine")
    parser.add_argument("--net-a", required=True)
    parser.add_argument("--net-b", required=True)
    parser.add_argument("--games", type=int, default=12)
    parser.add_argument("--movetime", type=int, default=100)
    parser.add_argument("--plies", type=int, default=60)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--opening", nargs="+", default=["startpos"])
    args = parser.parse_args()

    engine_a = configured(args.engine, args.net_a)
    engine_b = configured(args.engine, args.net_b)
    rng = random.Random(args.seed + int(time.time()))
    startpos = chess.Board()

    # Deterministic pseudo-random openings from a small pool.
    opening_pool = [
        [],  # startpos
        [chess.Move.from_uci("e2e4"), chess.Move.from_uci("e7e5"),
         chess.Move.from_uci("g1f3"), chess.Move.from_uci("b8c6")],
        [chess.Move.from_uci("d2d4"), chess.Move.from_uci("d7d5"),
         chess.Move.from_uci("c2c4"), chess.Move.from_uci("e7e6")],
        [chess.Move.from_uci("c2c4"), chess.Move.from_uci("e7e5"),
         chess.Move.from_uci("g1f3"), chess.Move.from_uci("g8f6")],
        [chess.Move.from_uci("e2e4"), chess.Move.from_uci("c7c5"),
         chess.Move.from_uci("g1f3"), chess.Move.from_uci("d7d6")],
        [chess.Move.from_uci("g1f3"), chess.Move.from_uci("g8f6"),
         chess.Move.from_uci("c2c4"), chess.Move.from_uci("e7e6")],
    ]

    limit = chess.engine.Limit(time=args.movetime / 1000.0)
    results: list[str] = []
    games_played = 0

    for game_idx in range(args.games):
        opening = rng.choice(opening_pool)
        board = chess.Board()
        for move in opening:
            board.push(move)
        # Color alternation: A is white on even games.
        a_white = (game_idx % 2 == 0)
        try:
            while not board.is_game_over() and board.ply() <= args.plies:
                engine = engine_a if (board.turn == chess.WHITE) == a_white else engine_b
                result = engine.play(board, limit)
                board.push(result.move)
            if board.is_game_over():
                outcome = board.result()
            else:
                try:
                    outcome = board.result(claim_draw=True)
                except ValueError:
                    outcome = "1/2-1/2"
            results.append(outcome)
            games_played += 1
        except (chess.engine.EngineError, EOFError):
            print(f"WARN: game {game_idx} interrupted")
            continue

    score_a = sum(1.0 for r in results if r == "1-0") + 0.5 * sum(
        1.0 for r in results if r == "1/2-1/2"
    )
    print(f"Games: {games_played}")
    print(f"Results: {' '.join(results)}")
    print(f"Net A ({args.net_a}) score: {score_a:.1f}/{games_played}")
    print(f"Net B ({args.net_b}) score: {games_played - score_a:.1f}/{games_played}")

    engine_a.quit()
    engine_b.quit()


if __name__ == "__main__":
    main()