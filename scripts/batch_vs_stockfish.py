#!/usr/bin/env python3
"""
Run multiple short matches vs Stockfish using play_vs_stockfish.py and save PGNs.

This avoids python-chess engine integration by parsing the moves line emitted by
play_vs_stockfish.py, then reconstructing the game with python-chess.
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import subprocess
import sys
from pathlib import Path

import chess
import chess.pgn


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/debug/chess_engine", help="Path to our UCI engine")
    parser.add_argument("--stockfish", default="/usr/games/stockfish", help="Path to Stockfish binary")
    parser.add_argument("--games", type=int, default=6, help="Number of games to run")
    parser.add_argument("--plies", type=int, default=40, help="Plies per game")
    parser.add_argument("--movetime", type=int, default=50, help="Movetime per move in ms")
    parser.add_argument("--pgn-out", type=Path, required=True, help="Output PGN file (append)")
    parser.add_argument(
        "--engine-color",
        choices=["white", "black", "alternate"],
        default="alternate",
        help="Side for our engine",
    )
    parser.add_argument("--time-control", action="store_true", help="Use time control instead of movetime")
    parser.add_argument("--wtime", type=int, default=1000, help="White time in ms (time-control mode)")
    parser.add_argument("--btime", type=int, default=1000, help="Black time in ms (time-control mode)")
    parser.add_argument("--winc", type=int, default=0, help="White increment in ms (time-control mode)")
    parser.add_argument("--binc", type=int, default=0, help="Black increment in ms (time-control mode)")
    parser.add_argument("--movestogo", type=int, help="Moves to go (time-control mode)")
    return parser.parse_args()


def choose_color(mode: str, idx: int) -> str:
    if mode == "white":
        return "white"
    if mode == "black":
        return "black"
    return "white" if idx % 2 == 0 else "black"


def run_game(args: argparse.Namespace, engine_color: str) -> list[str]:
    env = os.environ.copy()
    env["ENGINE"] = args.engine if engine_color == "white" else args.stockfish
    env["STOCKFISH"] = args.stockfish if engine_color == "white" else args.engine
    env["PLIES"] = str(args.plies)
    if args.time_control:
        env["WTIME_MS"] = str(args.wtime)
        env["BTIME_MS"] = str(args.btime)
        env["WINC_MS"] = str(args.winc)
        env["BINC_MS"] = str(args.binc)
        if args.movestogo is not None:
            env["MOVESTOGO"] = str(args.movestogo)
    else:
        env["MOVETIME_MS"] = str(args.movetime)

    result = subprocess.run(
        [sys.executable, "scripts/play_vs_stockfish.py"],
        cwd=Path.cwd(),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=True,
    )
    for line in result.stdout.splitlines():
        if line.startswith("moves: "):
            moves = line.split("moves: ", 1)[1].strip()
            return moves.split() if moves else []
    raise RuntimeError("No moves line found in play_vs_stockfish.py output")


def moves_to_pgn(moves: list[str], engine_color: str) -> chess.pgn.Game:
    board = chess.Board()
    game = chess.pgn.Game()
    game.headers["Event"] = "vs Stockfish (batch)"
    game.headers["Site"] = "Local"
    game.headers["Date"] = dt.date.today().strftime("%Y.%m.%d")
    game.headers["Round"] = "1"
    if engine_color == "white":
        game.headers["White"] = "chess_engine"
        game.headers["Black"] = "stockfish"
    else:
        game.headers["White"] = "stockfish"
        game.headers["Black"] = "chess_engine"

    node = game
    for mv in moves:
        move = chess.Move.from_uci(mv)
        if move not in board.legal_moves:
            raise RuntimeError(f"illegal move: {mv}")
        board.push(move)
        node = node.add_variation(move)

    if board.is_game_over(claim_draw=True):
        game.headers["Result"] = board.result(claim_draw=True)
    else:
        game.headers["Result"] = "*"
        game.headers["Termination"] = "incomplete"
    return game


def main() -> int:
    args = parse_args()
    args.pgn_out.parent.mkdir(parents=True, exist_ok=True)
    summary = []

    for i in range(args.games):
        color = choose_color(args.engine_color, i)
        moves = run_game(args, color)
        game = moves_to_pgn(moves, color)
        with open(args.pgn_out, "a", encoding="utf-8") as f:
            print(game, file=f, end="\n\n")
        summary.append((i + 1, color, len(moves), game.headers["Result"]))
        print(f"game {i+1}: color={color} plies={len(moves)} result={game.headers['Result']}")

    wins = sum(1 for _, c, _, r in summary if r == ("1-0" if c == "white" else "0-1"))
    losses = sum(1 for _, c, _, r in summary if r == ("0-1" if c == "white" else "1-0"))
    draws = sum(1 for _, _, _, r in summary if r == "1/2-1/2")
    incomplete = sum(1 for _, _, _, r in summary if r == "*")
    print(f"summary: wins={wins} losses={losses} draws={draws} incomplete={incomplete}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
