#!/usr/bin/env python3
"""Create weighted child-position ranking pairs from root MultiPV teacher scores."""

import argparse
import queue
import re
import subprocess
import threading
import time

import chess


SCORE_CP_RE = re.compile(r"\bscore cp (-?\d+)")
SCORE_MATE_RE = re.compile(r"\bscore mate (-?\d+)")
MULTIPV_RE = re.compile(r"\bmultipv (\d+)")
DEPTH_RE = re.compile(r"\bdepth (\d+)")


class UCIEngine:
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

    def _reader(self):
        for line in self.proc.stdout:
            self.lines.put(line.rstrip("\n"))

    def send(self, cmd: str):
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def wait_for(self, token: str, timeout: float = 10.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if token in line:
                return line
        raise RuntimeError(f"timeout waiting for {token}")

    @staticmethod
    def _score_from_info(line: str) -> int | None:
        if match := SCORE_CP_RE.search(line):
            return int(match.group(1))
        if match := SCORE_MATE_RE.search(line):
            mate = int(match.group(1))
            score = 30000 - min(abs(mate), 300) * 100
            return score if mate > 0 else -score
        return None

    def multipv_scores(self, fen: str, depth: int, movetime_ms: int) -> dict[chess.Move, int]:
        self.send("ucinewgame")
        self.send(f"position fen {fen}")
        if depth > 0:
            self.send(f"go depth {depth}")
            timeout = 60.0
        else:
            self.send(f"go movetime {movetime_ms}")
            timeout = movetime_ms / 1000 + 10

        by_slot: dict[int, tuple[int, int, chess.Move]] = {}
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue

            if line.startswith("info ") and " pv " in line:
                pv = line.split(" pv ", 1)[1].split()
                if not pv:
                    continue
                try:
                    move = chess.Move.from_uci(pv[0])
                except ValueError:
                    continue
                score = self._score_from_info(line)
                if score is None:
                    continue
                depth_match = DEPTH_RE.search(line)
                slot_match = MULTIPV_RE.search(line)
                seen_depth = int(depth_match.group(1)) if depth_match else 0
                slot = int(slot_match.group(1)) if slot_match else 1
                old = by_slot.get(slot)
                if old is None or seen_depth >= old[0]:
                    by_slot[slot] = (seen_depth, score, move)

            if line.startswith("bestmove"):
                return {move: score for _slot, (_depth, score, move) in by_slot.items()}
        return {move: score for _slot, (_depth, score, move) in by_slot.items()}

    def quit(self):
        if self.proc.poll() is None:
            self.send("quit")
            try:
                self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def parse_epd(line: str):
    board = chess.Board()
    ops = board.set_epd(line.strip())
    return board, set(ops.get("bm", [])), ops.get("id", "unknown")


def child_fen(board: chess.Board, move: chess.Move) -> str:
    child = board.copy(stack=False)
    child.push(move)
    return child.fen()


def weight_from_gap(gap_cp: int, min_gap_cp: int, max_weight: float) -> float:
    if gap_cp < min_gap_cp:
        return 0.0
    return min(max_weight, 1.0 + gap_cp / 100.0)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--epd", nargs="+", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--movetime-ms", type=int, default=0)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--min-gap-cp", type=int, default=25)
    parser.add_argument("--max-weight", type=float, default=6.0)
    parser.add_argument("--prefer-epd-bm", action="store_true")
    parser.add_argument("--setoption", action="append", default=[])
    args = parser.parse_args()

    engine = UCIEngine(args.engine, args.setoption)
    roots = scored_roots = pairs = 0
    try:
        with open(args.output, "w") as out:
            for path in args.epd:
                with open(path) as f:
                    for line in f:
                        if not line.strip():
                            continue
                        if args.limit and roots >= args.limit:
                            break
                        try:
                            board, bm_set, _epd_id = parse_epd(line)
                        except Exception:
                            continue
                        roots += 1
                        legal = list(board.legal_moves)
                        if len(legal) < 2:
                            continue

                        # Make MultiPV cover all legal moves where possible.
                        engine.send(f"setoption name MultiPV value {min(64, len(legal))}")
                        engine.send("isready")
                        engine.wait_for("readyok")
                        scores = engine.multipv_scores(board.fen(), args.depth, args.movetime_ms)
                        if not scores:
                            continue
                        scored_roots += 1

                        legal_scored = [mv for mv in legal if mv in scores]
                        if len(legal_scored) < 2:
                            continue

                        positives = [mv for mv in legal_scored if mv in bm_set] if args.prefer_epd_bm else []
                        if not positives:
                            best_score = max(scores[mv] for mv in legal_scored)
                            positives = [mv for mv in legal_scored if scores[mv] == best_score]

                        for best in positives:
                            best_score = scores[best]
                            best_child = child_fen(board, best)
                            for other in legal_scored:
                                if other == best or other in positives:
                                    continue
                                gap = best_score - scores[other]
                                weight = weight_from_gap(gap, args.min_gap_cp, args.max_weight)
                                if weight <= 0.0:
                                    continue
                                out.write(f"{best_child} | {child_fen(board, other)} | {weight:.3f}\n")
                                pairs += 1
                if args.limit and roots >= args.limit:
                    break
    finally:
        engine.quit()
    print(f"Scored {scored_roots:,}/{roots:,} roots; wrote {pairs:,} weighted pairs to {args.output}")


if __name__ == "__main__":
    main()
