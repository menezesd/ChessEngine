#!/usr/bin/env python3
"""Quick WAC test with NNUE option."""

import argparse
from contextlib import ExitStack

import chess

if __package__:
    from .uci_client import UCIClient
else:
    from uci_client import UCIClient


class UCIEngine(UCIClient):
    def __init__(self, path: str, nnue_file: str = None, options: list[str] = None):
        configured = []
        if nnue_file:
            configured += [f"EvalFile value {nnue_file}", "UseNNUE value true"]
        super().__init__(path, [*configured, *(options or [])])

    _send = UCIClient.send
    _drain_until = UCIClient.wait_for

    def bestmove(self, fen, movetime_ms, depth=0):
        self.send("ucinewgame")
        self.send(f"position fen {fen}")
        command = f"go depth {depth}" if depth > 0 else f"go movetime {movetime_ms}"
        timeout = 60 if depth > 0 else movetime_ms / 1000 + 10
        for line in self.search_lines(command, timeout):
            if line.startswith("bestmove "):
                return line.split()[1]


def parse_epd(line):
    board = chess.Board()
    operations = board.set_epd(line)
    if not operations.get("bm"):
        return None, None, None
    return board.fen(), [board.san(move) for move in operations["bm"]], operations.get("id", "unknown")


def iter_epd(path, limit):
    total = 0
    with open(path) as source:
        for line in source:
            if total >= limit:
                break
            if not line.strip():
                continue
            parsed = parse_epd(line)
            if parsed[0] is not None:
                total += 1
                yield parsed


def uci_to_san(fen, uci):
    try:
        board = chess.Board(fen)
        move = chess.Move.from_uci(uci)
        return board.san(move)
    except (ValueError, AssertionError):
        return uci


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--epd", default="tests/data/wac_all.epd")
    parser.add_argument("--limit", type=int, default=50)
    parser.add_argument("--movetime-ms", type=int, default=500)
    parser.add_argument("--depth", type=int, default=0)
    parser.add_argument("--nnue", default=None, help="Path to NNUE file")
    parser.add_argument(
        "--setoption",
        action="append",
        default=[],
        help="UCI option in the form 'Name value X'; may be repeated",
    )
    parser.add_argument("--print-misses", action="store_true")
    args = parser.parse_args()

    total = correct = 0
    with ExitStack() as cleanup:
        engine = UCIEngine(args.engine, args.nnue, args.setoption)
        cleanup.callback(engine.quit)
        for fen, bms, epd_id in iter_epd(args.epd, args.limit):
            total += 1
            best = engine.bestmove(fen, args.movetime_ms, args.depth)
            san = uci_to_san(fen, best)
            if san in bms or best in bms:
                correct += 1
            elif args.print_misses:
                print(f"MISS {epd_id}: best={best}/{san} expected={','.join(bms)}")

    use_nnue = args.nnue is not None or any(
        option.lower().replace(" ", "") == "usennuevaluetrue"
        for option in args.setoption
    )
    mode = "NNUE" if use_nnue else "HCE"
    print(f"WAC ({mode}): {correct}/{total}")


if __name__ == "__main__":
    main()
