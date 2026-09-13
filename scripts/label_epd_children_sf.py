#!/usr/bin/env python3
"""Label legal child positions from EPD tactical roots with Stockfish."""

import argparse
from concurrent.futures import ProcessPoolExecutor, as_completed

import chess


if __package__:
    from .uci_client import UCIClient
else:
    from uci_client import UCIClient


def parse_epd(line: str):
    board = chess.Board()
    ops = board.set_epd(line.strip())
    return board, ops.get("bm", [])


def result_from_white_eval(eval_cp: int) -> float:
    if eval_cp > 100:
        return 1.0
    if eval_cp < -100:
        return 0.0
    return 0.5


def collect_children(epd_paths: list[str], limit_roots: int, include_all: bool, best_repeat: int):
    rows: list[tuple[str, float]] = []
    roots = 0
    for path in epd_paths:
        with open(path) as f:
            for line in f:
                if not line.strip():
                    continue
                if limit_roots and roots >= limit_roots:
                    return rows
                roots += 1
                try:
                    board, best_moves = parse_epd(line)
                except Exception:
                    continue
                best_set = set(best_moves)
                for move in board.legal_moves:
                    if not include_all and move not in best_set:
                        continue
                    child = board.copy(stack=False)
                    child.push(move)
                    repeat = best_repeat if move in best_set else 1
                    rows.extend([(child.fen(), result_from_move_result(child))] * repeat)
    return rows


def result_from_move_result(_board: chess.Board) -> float:
    # Placeholder game result; eval label is the important target for these rows.
    return 0.5


class Stockfish(UCIClient):
    def __init__(self, path: str, depth: int):
        self.depth = depth
        super().__init__(path, ["Threads value 1", "Hash value 64"])

    wait = UCIClient.wait_for

    def eval(self, fen: str) -> int | None:
        return self.score_position(fen, f"go depth {self.depth}", new_game=False)[0]


def label_batch(batch, sf_path: str, depth: int):
    sf = Stockfish(sf_path, depth)
    labeled = []
    try:
        for fen, game_result in batch:
            score = sf.eval(fen)
            if score is None:
                continue
            board = chess.Board(fen)
            white_eval = score if board.turn == chess.WHITE else -score
            labeled.append(f"{fen} | {white_eval} | {game_result:.1f}")
    finally:
        sf.quit()
    return labeled


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--epd", nargs="+", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--sf-path", default="/opt/local/bin/stockfish")
    parser.add_argument("--depth", type=int, default=10)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--batch-size", type=int, default=128)
    parser.add_argument("--limit-roots", type=int, default=0)
    parser.add_argument("--best-only", action="store_true")
    parser.add_argument("--best-repeat", type=int, default=1)
    args = parser.parse_args()

    children = collect_children(
        args.epd,
        args.limit_roots,
        include_all=not args.best_only,
        best_repeat=args.best_repeat,
    )
    print(f"Collected {len(children):,} child positions")

    batches = [children[i : i + args.batch_size] for i in range(0, len(children), args.batch_size)]
    labeled = []
    with ProcessPoolExecutor(max_workers=args.workers) as pool:
        futures = [pool.submit(label_batch, batch, args.sf_path, args.depth) for batch in batches]
        for i, fut in enumerate(as_completed(futures), 1):
            labeled.extend(fut.result())
            if i % 10 == 0:
                print(f"  labeled {len(labeled):,}/{len(children):,}", flush=True)

    with open(args.output, "w") as f:
        for line in labeled:
            f.write(line + "\n")
    print(f"Wrote {len(labeled):,} rows to {args.output}")


if __name__ == "__main__":
    main()
