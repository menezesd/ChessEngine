#!/usr/bin/env python3
"""Generate FENs from positions likely to be used as static/qsearch evals."""

import argparse
import random

import chess


def parse_position(line: str) -> chess.Board | None:
    line = line.strip()
    if not line:
        return None
    if "|" in line:
        line = line.split("|", 1)[0].strip()
    try:
        return chess.Board(line)
    except Exception:
        pass
    try:
        board = chess.Board()
        board.set_epd(line)
        return board
    except Exception:
        return None


def is_tactical(board: chess.Board, move: chess.Move) -> bool:
    return board.is_capture(move) or move.promotion is not None or board.gives_check(move)


def add_board(board: chess.Board, out: list[str], seen: set[str], max_positions: int) -> bool:
    if board.is_game_over(claim_draw=True):
        return False
    fen = board.fen()
    if fen in seen:
        return False
    seen.add(fen)
    out.append(fen)
    return bool(max_positions and len(out) >= max_positions)


def add_child(board: chess.Board, move: chess.Move, out: list[str], seen: set[str], max_positions: int) -> bool:
    child = board.copy(stack=False)
    child.push(move)
    return add_board(child, out, seen, max_positions)


def add_random_walks(
    board: chess.Board,
    rng: random.Random,
    out: list[str],
    seen: set[str],
    max_positions: int,
    walks: int,
    max_plies: int,
) -> bool:
    for _ in range(walks):
        child = board.copy(stack=False)
        plies = rng.randint(1, max_plies)
        for _ply in range(plies):
            legal = list(child.legal_moves)
            if not legal:
                break
            tactical = [move for move in legal if is_tactical(child, move)]
            # Bias toward qsearch-like tactical branches without excluding quiet leaves.
            candidates = tactical if tactical and rng.random() < 0.7 else legal
            child.push(rng.choice(candidates))
            if add_board(child, out, seen, max_positions):
                return True
    return False


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", nargs="+", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--max-positions", type=int, default=100_000)
    parser.add_argument("--roots", type=int, default=0)
    parser.add_argument("--legal-children", type=int, default=4)
    parser.add_argument("--tactical-children", type=int, default=16)
    parser.add_argument("--random-walks", type=int, default=2)
    parser.add_argument("--max-random-plies", type=int, default=4)
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    rng = random.Random(args.seed)
    positions: list[str] = []
    seen: set[str] = set()
    roots = 0

    for path in args.input:
        with open(path) as f:
            for line in f:
                board = parse_position(line)
                if board is None:
                    continue
                roots += 1
                if args.roots and roots > args.roots:
                    break

                if add_board(board, positions, seen, args.max_positions):
                    break

                legal = list(board.legal_moves)
                rng.shuffle(legal)

                tactical = [move for move in legal if is_tactical(board, move)]
                quiet = [move for move in legal if move not in tactical]

                for move in tactical[: args.tactical_children]:
                    if add_child(board, move, positions, seen, args.max_positions):
                        break
                if args.max_positions and len(positions) >= args.max_positions:
                    break

                for move in quiet[: args.legal_children]:
                    if add_child(board, move, positions, seen, args.max_positions):
                        break
                if args.max_positions and len(positions) >= args.max_positions:
                    break

                if add_random_walks(
                    board,
                    rng,
                    positions,
                    seen,
                    args.max_positions,
                    args.random_walks,
                    args.max_random_plies,
                ):
                    break

                if roots % 10_000 == 0:
                    print(
                        f"  roots {roots:,}, positions {len(positions):,}",
                        flush=True,
                    )

        if args.max_positions and len(positions) >= args.max_positions:
            break
        if args.roots and roots >= args.roots:
            break

    with open(args.output, "w") as out:
        for fen in positions:
            out.write(fen + "\n")

    print(f"Wrote {len(positions):,} positions from {roots:,} roots to {args.output}")


if __name__ == "__main__":
    main()
