#!/usr/bin/env python3
"""
Play two instances of the same UCI engine against each other with different env settings.
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


class UCIEngine:
    def __init__(self, path: str, name: str, env_overrides: dict[str, str] | None = None):
        env = os.environ.copy()
        if env_overrides:
            env.update(env_overrides)
        self.proc = subprocess.Popen(
            [path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            env=env,
        )
        self.name = name
        self._handshake()

    def _send(self, cmd: str) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def _readline(self) -> str:
        assert self.proc.stdout is not None
        line = self.proc.stdout.readline()
        if not line:
            return ""
        return line.strip()

    def _drain_until(self, token: str, timeout_lines: int = 1000) -> None:
        for _ in range(timeout_lines):
            line = self._readline()
            if token in line:
                return
        raise RuntimeError(f"{self.name}: timeout waiting for {token}")

    def _handshake(self) -> None:
        self._send("uci")
        self._drain_until("uciok")
        self._send("isready")
        self._drain_until("readyok")

    def bestmove(self, moves: list[str], movetime_ms: int) -> str:
        position = "position startpos"
        if moves:
            position += " moves " + " ".join(moves)
        self._send(position)
        self._send(f"go movetime {movetime_ms}")
        while True:
            line = self._readline()
            if line.startswith("bestmove "):
                return line.split()[1]

    def quit(self) -> None:
        if self.proc.poll() is None:
            self._send("quit")
            try:
                self.proc.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                self.proc.kill()


class ScoreEngine:
    def __init__(self, path: str):
        self.proc = subprocess.Popen(
            [path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        self._handshake()

    def _send(self, cmd: str) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def _readline(self) -> str:
        assert self.proc.stdout is not None
        line = self.proc.stdout.readline()
        if not line:
            return ""
        return line.strip()

    def _drain_until(self, token: str, timeout_lines: int = 2000) -> None:
        for _ in range(timeout_lines):
            line = self._readline()
            if token in line:
                return
        raise RuntimeError(f"timeout waiting for {token}")

    def _handshake(self) -> None:
        self._send("uci")
        self._drain_until("uciok")
        self._send("isready")
        self._drain_until("readyok")

    def evaluate_fen(self, fen: str, movetime_ms: int) -> int | None:
        self._send(f"position fen {fen}")
        self._send(f"go movetime {movetime_ms}")
        last_score = None
        while True:
            line = self._readline()
            if line.startswith("info") and "score" in line:
                parts = line.split()
                if "score" in parts:
                    idx = parts.index("score")
                    if idx + 2 < len(parts):
                        kind = parts[idx + 1]
                        val = parts[idx + 2]
                        if kind == "cp":
                            try:
                                last_score = int(val)
                            except ValueError:
                                pass
                        elif kind == "mate":
                            try:
                                mate = int(val)
                                last_score = 100000 if mate > 0 else -100000
                            except ValueError:
                                pass
            if line.startswith("bestmove "):
                return last_score

    def quit(self) -> None:
        if self.proc.poll() is None:
            self._send("quit")
            try:
                self.proc.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def parse_env_list(raw: str) -> dict[str, str]:
    env = {}
    if not raw:
        return env
    for entry in raw.split(";"):
        if "=" not in entry:
            continue
        key, val = entry.split("=", 1)
        env[key.strip()] = val.strip()
    return env


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/debug/chess_engine", help="Path to UCI engine")
    parser.add_argument("--games", type=int, default=8, help="Number of games")
    parser.add_argument("--plies", type=int, default=40, help="Plies per game")
    parser.add_argument("--movetime", type=int, default=200, help="Movetime per move in ms")
    parser.add_argument("--pgn-out", type=Path, required=True, help="Output PGN file (append)")
    parser.add_argument("--engine-a-name", default="engine_a", help="Name for engine A")
    parser.add_argument("--engine-b-name", default="engine_b", help="Name for engine B")
    parser.add_argument("--engine-a-env", default="", help="Env overrides for engine A (KEY=VAL;KEY=VAL)")
    parser.add_argument("--engine-b-env", default="", help="Env overrides for engine B (KEY=VAL;KEY=VAL)")
    parser.add_argument("--stockfish", default="/usr/games/stockfish", help="Path to Stockfish binary")
    parser.add_argument("--adjudicate", action="store_true", help="Use Stockfish adjudication")
    parser.add_argument("--adjudicate-plies", type=int, default=12, help="Check adjudication every N plies")
    parser.add_argument("--adjudicate-cp", type=int, default=200, help="CP threshold for adjudication")
    parser.add_argument("--adjudicate-movetime", type=int, default=80, help="Stockfish movetime in ms")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    a_env = parse_env_list(args.engine_a_env)
    b_env = parse_env_list(args.engine_b_env)
    args.pgn_out.parent.mkdir(parents=True, exist_ok=True)

    summary = {"a_win": 0, "b_win": 0, "draw": 0, "incomplete": 0}

    for g in range(args.games):
        a_is_white = g % 2 == 0
        engine_a = UCIEngine(args.engine, args.engine_a_name, env_overrides=a_env)
        engine_b = UCIEngine(args.engine, args.engine_b_name, env_overrides=b_env)
        board = chess.Board()
        moves: list[str] = []
        result_override = None
        sf = ScoreEngine(args.stockfish) if args.adjudicate else None
        try:
            for _ in range(args.plies):
                side = engine_a if (board.turn and a_is_white) or (not board.turn and not a_is_white) else engine_b
                move = side.bestmove(moves, movetime_ms=args.movetime)
                if move in ("0000", "(none)"):
                    break
                board.push_uci(move)
                moves.append(move)
                if board.is_game_over(claim_draw=True):
                    break
                if args.adjudicate and sf is not None and (board.ply() % max(1, args.adjudicate_plies) == 0):
                    score = sf.evaluate_fen(board.fen(), args.adjudicate_movetime)
                    if score is not None and score >= args.adjudicate_cp:
                        result_override = "1-0"
                        break
                    if score is not None and score <= -args.adjudicate_cp:
                        result_override = "0-1"
                        break
        finally:
            engine_a.quit()
            engine_b.quit()
            if sf is not None:
                sf.quit()

        game = chess.pgn.Game()
        game.headers["Event"] = "Head to Head"
        game.headers["Site"] = "Local"
        game.headers["Date"] = dt.date.today().strftime("%Y.%m.%d")
        game.headers["Round"] = str(g + 1)
        if a_is_white:
            game.headers["White"] = args.engine_a_name
            game.headers["Black"] = args.engine_b_name
        else:
            game.headers["White"] = args.engine_b_name
            game.headers["Black"] = args.engine_a_name

        node = game
        replay = chess.Board()
        for mv in moves:
            move = chess.Move.from_uci(mv)
            if move not in replay.legal_moves:
                break
            replay.push(move)
            node = node.add_variation(move)

        if result_override:
            result = result_override
            game.headers["Termination"] = "adjudication"
        elif replay.is_game_over(claim_draw=True):
            result = replay.result(claim_draw=True)
        else:
            result = "*"
            game.headers["Termination"] = "incomplete"
        game.headers["Result"] = result

        if result == "1-0":
            if a_is_white:
                summary["a_win"] += 1
            else:
                summary["b_win"] += 1
        elif result == "0-1":
            if a_is_white:
                summary["b_win"] += 1
            else:
                summary["a_win"] += 1
        elif result == "1/2-1/2":
            summary["draw"] += 1
        else:
            summary["incomplete"] += 1

        with open(args.pgn_out, "a", encoding="utf-8") as f:
            print(game, file=f, end="\n\n")
        print(
            f"game {g+1}: a_is_white={a_is_white} plies={len(moves)} result={result}"
        )

    print(
        f"summary: a_win={summary['a_win']} b_win={summary['b_win']} "
        f"draw={summary['draw']} incomplete={summary['incomplete']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
