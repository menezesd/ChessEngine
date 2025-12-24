#!/usr/bin/env python3
"""
Run a small parameter sweep with self-play using env-driven material values.

Candidates play against the baseline (default material values) with alternating colors.
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import chess
import chess.pgn


BASE_MG = [82, 337, 365, 477, 1025, 20000]
BASE_EG = [94, 281, 297, 512, 936, 20000]


@dataclass(frozen=True)
class Candidate:
    name: str
    mg: list[int]
    eg: list[int]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/debug/chess_engine", help="Path to UCI engine")
    parser.add_argument("--stockfish", default="/usr/games/stockfish", help="Path to Stockfish binary")
    parser.add_argument("--games-per-candidate", type=int, default=2, help="Games per candidate")
    parser.add_argument("--plies", type=int, default=30, help="Plies per game")
    parser.add_argument("--movetime", type=int, default=200, help="Movetime per move in ms")
    parser.add_argument("--pgn-out", type=Path, help="Optional PGN output file (append)")
    parser.add_argument("--adjudicate", action="store_true", help="Use Stockfish adjudication")
    parser.add_argument("--adjudicate-plies", type=int, default=12, help="Check adjudication every N plies")
    parser.add_argument("--adjudicate-cp", type=int, default=200, help="CP threshold for adjudication")
    parser.add_argument("--adjudicate-movetime", type=int, default=80, help="Stockfish movetime in ms")
    return parser.parse_args()


def scale_values(values: list[int], factor: float, idxs: list[int]) -> list[int]:
    scaled = values[:]
    for idx in idxs:
        scaled[idx] = int(round(values[idx] * factor))
    return scaled


def build_candidates() -> list[Candidate]:
    candidates = []
    candidates.append(Candidate("knight_up", scale_values(BASE_MG, 1.05, [1]), scale_values(BASE_EG, 1.05, [1])))
    candidates.append(
        Candidate("knight_down", scale_values(BASE_MG, 0.95, [1]), scale_values(BASE_EG, 0.95, [1]))
    )
    candidates.append(Candidate("bishop_up", scale_values(BASE_MG, 1.05, [2]), scale_values(BASE_EG, 1.05, [2])))
    candidates.append(
        Candidate("bishop_down", scale_values(BASE_MG, 0.95, [2]), scale_values(BASE_EG, 0.95, [2]))
    )
    return candidates


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


def play_game(
    engine_path: str,
    candidate: Candidate,
    candidate_is_white: bool,
    movetime_ms: int,
    plies: int,
    adjudicate: bool,
    adjudicate_plies: int,
    adjudicate_cp: int,
    adjudicate_movetime: int,
    stockfish_path: str,
):
    candidate_env = {
        "EVAL_MATERIAL_MG": ",".join(str(v) for v in candidate.mg),
        "EVAL_MATERIAL_EG": ",".join(str(v) for v in candidate.eg),
    }
    cand = UCIEngine(engine_path, "candidate", env_overrides=candidate_env)
    base = UCIEngine(engine_path, "baseline")
    moves: list[str] = []
    board = chess.Board()
    result_override = None
    sf = None
    if adjudicate:
        sf = ScoreEngine(stockfish_path)

    try:
        for ply in range(plies):
            side = cand if (ply % 2 == 0) == candidate_is_white else base
            move = side.bestmove(moves, movetime_ms=movetime_ms)
            if move in ("0000", "(none)"):
                break
            board.push_uci(move)
            moves.append(move)

            if board.is_game_over(claim_draw=True):
                break

            if adjudicate and sf is not None:
                if (ply + 1) % max(1, adjudicate_plies) == 0:
                    score = sf.evaluate_fen(board.fen(), adjudicate_movetime)
                    if score is not None and score >= adjudicate_cp:
                        result_override = "1-0"
                        break
                    if score is not None and score <= -adjudicate_cp:
                        result_override = "0-1"
                        break
    finally:
        cand.quit()
        base.quit()
        if sf is not None:
            sf.quit()

    game = chess.pgn.Game()
    game.headers["Event"] = f"selfplay sweep {candidate.name}"
    game.headers["Site"] = "Local"
    game.headers["Date"] = dt.date.today().strftime("%Y.%m.%d")
    game.headers["Round"] = "1"
    if candidate_is_white:
        game.headers["White"] = f"candidate({candidate.name})"
        game.headers["Black"] = "baseline"
    else:
        game.headers["White"] = "baseline"
        game.headers["Black"] = f"candidate({candidate.name})"

    node = game
    replay = chess.Board()
    for mv in moves:
        move = chess.Move.from_uci(mv)
        if move not in replay.legal_moves:
            break
        replay.push(move)
        node = node.add_variation(move)

    if result_override:
        game.headers["Result"] = result_override
        game.headers["Termination"] = "adjudication"
    elif replay.is_game_over(claim_draw=True):
        game.headers["Result"] = replay.result(claim_draw=True)
    else:
        game.headers["Result"] = "*"
        game.headers["Termination"] = "incomplete"
    return game, candidate_is_white


def score_result(result: str, candidate_is_white: bool) -> str:
    if result == "1/2-1/2":
        return "draw"
    if result == "*":
        return "incomplete"
    if result == "1-0":
        return "win" if candidate_is_white else "loss"
    if result == "0-1":
        return "loss" if candidate_is_white else "win"
    return "incomplete"


def main() -> int:
    args = parse_args()
    candidates = build_candidates()
    summary = {}

    for candidate in candidates:
        tally = {"win": 0, "loss": 0, "draw": 0, "incomplete": 0}
        for g in range(args.games_per_candidate):
            candidate_is_white = g % 2 == 0
            game, cand_white = play_game(
                args.engine,
                candidate,
                candidate_is_white,
                args.movetime,
                args.plies,
                args.adjudicate,
                args.adjudicate_plies,
                args.adjudicate_cp,
                args.adjudicate_movetime,
                args.stockfish,
            )
            result = score_result(game.headers["Result"], cand_white)
            tally[result] += 1
            if args.pgn_out:
                args.pgn_out.parent.mkdir(parents=True, exist_ok=True)
                with open(args.pgn_out, "a", encoding="utf-8") as f:
                    print(game, file=f, end="\n\n")
            print(
                f"{candidate.name} game {g+1}: color={'white' if cand_white else 'black'} "
                f"plies={len(list(game.mainline_moves()))} result={game.headers['Result']}"
            )
        summary[candidate.name] = tally

    print("summary:")
    for name, tally in summary.items():
        print(
            f"  {name}: win={tally['win']} loss={tally['loss']} "
            f"draw={tally['draw']} incomplete={tally['incomplete']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
