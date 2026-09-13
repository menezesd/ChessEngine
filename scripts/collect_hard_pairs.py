#!/usr/bin/env python3
"""Collect ranking pairs from EPD positions missed by a UCI engine."""

import argparse

import chess

if __package__:
    from .uci_client import UCIClient
else:
    from uci_client import UCIClient


class UCIEngine(UCIClient):
    def bestmove(self, fen: str, movetime_ms: int) -> chess.Move | None:
        board = chess.Board(fen)
        self.send("ucinewgame")
        self.send(f"position fen {fen}")
        for line in self.search_lines(f"go movetime {movetime_ms}", movetime_ms / 1000 + 10):
            if line.startswith("bestmove"):
                parts = line.split()
                if len(parts) < 2 or parts[1] in {"(none)", "0000"}:
                    return None
                try:
                    move = chess.Move.from_uci(parts[1])
                except ValueError:
                    return None
                return move if move in board.legal_moves else None
        return None


def parse_epd(line: str):
    board = chess.Board()
    ops = board.set_epd(line.strip())
    return board, set(ops.get("bm", [])), ops.get("id", "unknown")


def child_fen(board: chess.Board, move: chess.Move) -> str:
    child = board.copy(stack=False)
    child.push(move)
    return child.fen()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--epd", nargs="+", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--movetime-ms", type=int, default=300)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--setoption", action="append", default=[])
    args = parser.parse_args()

    engine = UCIEngine(args.engine, args.setoption)
    total = misses = written = 0
    try:
        with open(args.output, "w") as out:
            for path in args.epd:
                with open(path) as f:
                    for line in f:
                        if not line.strip():
                            continue
                        if args.limit and total >= args.limit:
                            break
                        try:
                            board, best_moves, _epd_id = parse_epd(line)
                        except Exception:
                            continue
                        total += 1
                        picked = engine.bestmove(board.fen(), args.movetime_ms)
                        if picked is None or picked in best_moves:
                            continue
                        misses += 1
                        expected = next(iter(best_moves), None)
                        if expected is None:
                            continue
                        out.write(f"{child_fen(board, expected)} | {child_fen(board, picked)}\n")
                        written += 1
                if args.limit and total >= args.limit:
                    break
    finally:
        engine.quit()
    print(f"Checked {total:,}, misses {misses:,}, wrote {written:,} pairs to {args.output}")


if __name__ == "__main__":
    main()
