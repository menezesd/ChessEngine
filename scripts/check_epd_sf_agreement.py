#!/usr/bin/env python3
"""Check how often Stockfish's best legal child agrees with EPD bm."""

import argparse

import chess


if __package__:
    from .uci_client import UCIClient
else:
    from uci_client import UCIClient


class Stockfish(UCIClient):
    def __init__(self, path: str, depth: int):
        self.depth = depth
        super().__init__(path, ["Threads value 1", "Hash value 128"])

    wait = UCIClient.wait_for

    def eval(self, fen: str) -> int | None:
        return self.score_position(fen, f"go depth {self.depth}", new_game=False)[0]


def parse_epd(line: str):
    board = chess.Board()
    ops = board.set_epd(line.strip())
    return board, set(ops.get("bm", [])), ops.get("id", "unknown")


def child_score_for_parent(board: chess.Board, sf: Stockfish, move: chess.Move) -> int | None:
    child = board.copy(stack=False)
    child.push(move)
    score_for_child_stm = sf.eval(child.fen())
    if score_for_child_stm is None:
        return None
    # Convert child STM score to parent STM score after making move.
    return -score_for_child_stm


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--epd", required=True)
    parser.add_argument("--sf-path", default="/opt/local/bin/stockfish")
    parser.add_argument("--depth", type=int, default=10)
    parser.add_argument("--limit", type=int, default=0)
    args = parser.parse_args()

    sf = Stockfish(args.sf_path, args.depth)
    total = agree = 0
    try:
        with open(args.epd) as f:
            for line in f:
                if not line.strip():
                    continue
                if args.limit and total >= args.limit:
                    break
                board, best_moves, epd_id = parse_epd(line)
                scored = []
                for move in board.legal_moves:
                    score = child_score_for_parent(board, sf, move)
                    if score is not None:
                        scored.append((score, move))
                if not scored:
                    continue
                scored.sort(reverse=True, key=lambda item: item[0])
                top_score, top_move = scored[0]
                expected = max(
                    ((score, move) for score, move in scored if move in best_moves),
                    default=None,
                    key=lambda item: item[0],
                )
                total += 1
                ok = top_move in best_moves
                agree += int(ok)
                if not ok and total <= 30:
                    exp_text = "none" if expected is None else f"{board.san(expected[1])}:{expected[0]}"
                    print(f"MISS {epd_id}: sf={board.san(top_move)}:{top_score} epd={exp_text}")
    finally:
        sf.quit()
    print(f"Agreement: {agree}/{total}")


if __name__ == "__main__":
    main()
