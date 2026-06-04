#!/usr/bin/env python3
"""Extract tactical EPD roots from xinyangz/chess-tactics-pgn format."""

import argparse
import re

import chess


TAG_RE = re.compile(r'\[(\w+)\s+"([^"]*)"\]')
MOVE_NUM_RE = re.compile(r"\d+\.(?:\.\.)?")


def parse_games(text: str):
    chunks = re.split(r"\n(?=\[Event )", text)
    for chunk in chunks:
        if not chunk.strip():
            continue
        tags = dict(TAG_RE.findall(chunk))
        fen = tags.get("FEN")
        if not fen:
            continue
        move_text = "\n".join(line for line in chunk.splitlines() if not line.startswith("["))
        yield tags, move_text


def san_tokens(move_text: str) -> list[str]:
    move_text = re.sub(r"\{[^}]*\}", " ", move_text)
    move_text = MOVE_NUM_RE.sub(" ", move_text)
    tokens = []
    for token in move_text.split():
        token = token.strip()
        if token in {"*", "1-0", "0-1", "1/2-1/2"}:
            continue
        if token.startswith("$"):
            continue
        tokens.append(token)
    return tokens


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pgn", default="data/tactics_xinyangz.pgn")
    parser.add_argument("--output", default="tests/data/tactics_xinyangz.epd")
    parser.add_argument("--limit", type=int, default=0)
    args = parser.parse_args()

    with open(args.pgn, encoding="utf-8", errors="replace") as f:
        text = f.read()

    total = written = skipped = 0
    with open(args.output, "w") as out:
        for tags, move_text in parse_games(text):
            total += 1
            if args.limit and written >= args.limit:
                break
            try:
                board = chess.Board(tags["FEN"])
                tokens = san_tokens(move_text)
                if len(tokens) < 2:
                    skipped += 1
                    continue

                setup_move = board.parse_san(tokens[0])
                board.push(setup_move)
                solver_move = board.parse_san(tokens[1])
                san = board.san(solver_move)
                epd_id = tags.get("Event", f"tactic.{total}")
                out.write(f'{board.epd()} bm {san}; id "{epd_id}";\n')
                written += 1
            except Exception:
                skipped += 1

    print(f"Read {total:,} games, wrote {written:,} EPD rows, skipped {skipped:,}")


if __name__ == "__main__":
    main()
