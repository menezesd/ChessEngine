#!/usr/bin/env python3
"""Convert tactics PGN mainlines into NNUE eval training rows."""

import argparse
import random
import re

import chess


TAG_RE = re.compile(r'\[(\w+)\s+"([^"]*)"\]')
MOVE_NUM_RE = re.compile(r"\d+\.(?:\.\.)?")


def parse_games(text: str):
    for chunk in re.split(r"\n(?=\[Event )", text):
        if not chunk.strip():
            continue
        tags = dict(TAG_RE.findall(chunk))
        if "FEN" not in tags:
            continue
        move_text = "\n".join(line for line in chunk.splitlines() if not line.startswith("["))
        yield tags, move_text


def san_tokens(move_text: str) -> list[str]:
    move_text = re.sub(r"\{[^}]*\}", " ", move_text)
    move_text = MOVE_NUM_RE.sub(" ", move_text)
    return [
        token
        for token in move_text.split()
        if token not in {"*", "1-0", "0-1", "1/2-1/2"} and not token.startswith("$")
    ]


def result_from_white_eval(eval_cp: int) -> float:
    if eval_cp > 100:
        return 1.0
    if eval_cp < -100:
        return 0.0
    return 0.5


def row(board: chess.Board, eval_for_stm: int) -> str:
    white_eval = eval_for_stm if board.turn == chess.WHITE else -eval_for_stm
    return f"{board.fen()} | {white_eval} | {result_from_white_eval(white_eval):.1f}"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pgn", default="data/tactics_xinyangz.pgn")
    parser.add_argument("--output", default="data/tactics_xinyangz_training.txt")
    parser.add_argument("--root-cp", type=int, default=450)
    parser.add_argument("--reply-cp", type=int, default=-250)
    parser.add_argument("--mate-cp", type=int, default=1800)
    parser.add_argument("--max-games", type=int, default=0)
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    with open(args.pgn, encoding="utf-8", errors="replace") as f:
        text = f.read()

    rows = []
    games = skipped = 0
    for tags, move_text in parse_games(text):
        if args.max_games and games >= args.max_games:
            break
        games += 1
        try:
            board = chess.Board(tags["FEN"])
            tokens = san_tokens(move_text)
            if len(tokens) < 2:
                skipped += 1
                continue

            # The first move is the setup move by the non-solver side. The tactic
            # root is the resulting position, where the solver is to move.
            board.push(board.parse_san(tokens[0]))
            rows.append(row(board, args.root_cp))

            for ply_index, token in enumerate(tokens[1:], start=1):
                move = board.parse_san(token)
                board.push(move)
                if board.is_checkmate():
                    eval_for_stm = -args.mate_cp
                elif ply_index % 2 == 1:
                    # Solver just moved, so the defender is to move and should be worse.
                    eval_for_stm = args.reply_cp
                else:
                    # Defender just replied, so the solver is to move and should still be better.
                    eval_for_stm = args.root_cp
                rows.append(row(board, eval_for_stm))
        except Exception:
            skipped += 1

    random.Random(args.seed).shuffle(rows)
    with open(args.output, "w") as f:
        for item in rows:
            f.write(item + "\n")
    print(f"Read {games:,} games, wrote {len(rows):,} rows, skipped {skipped:,}")


if __name__ == "__main__":
    main()
