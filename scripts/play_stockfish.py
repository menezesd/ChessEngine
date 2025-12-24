#!/usr/bin/env python3
"""
Quick helper to play a single game against system Stockfish and save PGN.

Example:
  python3 scripts/play_stockfish.py --movetime 200 --pgn-out vs_sf.pgn
"""

import argparse
import pathlib
import subprocess
import sys
import time

import chess
import chess.engine
import chess.pgn


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/debug/chess_engine", help="Path to our UCI engine")
    parser.add_argument("--stockfish", default="/usr/games/stockfish", help="Path to Stockfish binary")
    parser.add_argument("--movetime", type=int, default=200, help="Movetime per move in ms")
    parser.add_argument("--pgn-out", type=pathlib.Path, help="Optional PGN output file (append)")
    parser.add_argument("--weplay", choices=["white", "black"], default="white", help="Side for our engine")
    parser.add_argument("--max-plies", type=int, default=200, help="Maximum plies before adjudicating a draw")
    args = parser.parse_args()

    game = chess.pgn.Game()
    game.headers["Event"] = "vs Stockfish"
    game.headers["Site"] = "Local"
    game.headers["Date"] = time.strftime("%Y.%m.%d")
    game.headers["Round"] = "1"
    game.headers["White"] = "chess_engine" if args.weplay == "white" else "stockfish"
    game.headers["Black"] = "stockfish" if args.weplay == "white" else "chess_engine"

    board = chess.Board()
    node = game

    our = chess.engine.SimpleEngine.popen_uci(args.engine)
    sf = chess.engine.SimpleEngine.popen_uci(args.stockfish)
    limit = chess.engine.Limit(time=args.movetime / 1000.0)

    def pick_engine(turn_white: bool):
        if args.weplay == "white":
            return our if turn_white else sf
        else:
            return sf if turn_white else our

    try:
        while not board.is_game_over():
            engine = pick_engine(board.turn)
            result = engine.play(board, limit)
            board.push(result.move)
            node = node.add_variation(result.move)
            if board.ply() >= args.max_plies:
                game.headers["Result"] = "1/2-1/2"
                break
    finally:
        our.quit()
        sf.quit()

    if "Result" not in game.headers:
        game.headers["Result"] = board.result()

    print(game)
    if args.pgn_out:
        with open(args.pgn_out, "a", encoding="utf-8") as f:
            print(game, file=f, end="\n\n")


if __name__ == "__main__":
    main()
