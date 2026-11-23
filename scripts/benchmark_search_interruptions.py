#!/usr/bin/env python3
"""Compare the returned UCI move with completed PVs as search budgets vary."""

import argparse
from collections import Counter
from contextlib import nullcontext
import json
from pathlib import Path
from statistics import median
from time import perf_counter

import chess
import chess.engine


def positive_int(value):
    number = int(value)
    if number <= 0:
        raise argparse.ArgumentTypeError("expected a positive integer")
    return number


def load_positions(path, count):
    positions = []
    with path.open() as source:
        for line in source:
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            board, ops = chess.Board.from_epd(line)
            if not board.is_valid() or not ops.get("bm"):
                continue
            positions.append((board, {move.uci() for move in ops["bm"]}))
            if len(positions) == count:
                break
    return positions


def run_search(engine, board, limit):
    start = perf_counter()
    with engine.analysis(board, limit, game=object()) as search:
        iterations = [
            {"depth": info["depth"], "nodes": info.get("nodes"), "move": info["pv"][0].uci()}
            for info in search if "depth" in info and info.get("pv")
        ]
        move = search.wait().move
    return {
        "bestmove": move.uci() if move else None,
        "wall_ms": round((perf_counter() - start) * 1_000, 2),
        "iterations": iterations,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--epd", type=Path, required=True, help="EPD with expected moves in bm")
    parser.add_argument("--positions", type=positive_int)
    parser.add_argument("--nodes", nargs="+", type=positive_int, default=[])
    parser.add_argument("--movetime-ms", nargs="+", type=positive_int, default=[])
    parser.add_argument("--clock-ms", nargs="+", type=positive_int, default=[],
                        help="time remaining per side with ten moves to go")
    parser.add_argument("--reference-depth", type=positive_int, default=11)
    parser.add_argument("--reference-nodes", type=positive_int, default=500_000)
    parser.add_argument("--move-overhead-ms", type=int, default=0)
    parser.add_argument("--hash-mb", type=positive_int, default=16)
    parser.add_argument("--output", type=Path, help="write per-position traces as JSONL")
    args = parser.parse_args()

    budgets = {
        ("nodes", nodes): chess.engine.Limit(nodes=nodes)
        for nodes in args.nodes
    }
    budgets.update(
        (("movetime_ms", ms), chess.engine.Limit(time=ms / 1_000))
        for ms in args.movetime_ms
    )
    budgets.update(
        (("clock_ms", ms), chess.engine.Limit(
            white_clock=ms / 1_000, black_clock=ms / 1_000, remaining_moves=10,
        ))
        for ms in args.clock_ms
    )
    if not budgets:
        parser.error("supply --nodes, --movetime-ms, or --clock-ms")
    if not 0 <= args.move_overhead_ms <= 1_000:
        parser.error("--move-overhead-ms must be between 0 and 1000")
    positions = load_positions(args.epd, args.positions)
    if not positions:
        parser.error("EPD has no valid positions with a bm operation")

    reference = chess.engine.Limit(depth=args.reference_depth, nodes=args.reference_nodes)
    totals = {key: Counter() for key in budgets}
    matches = []
    with (
        chess.engine.SimpleEngine.popen_uci(str(args.engine.resolve())) as engine,
        (args.output.open("w") if args.output else nullcontext()) as output,
    ):
        engine.configure({
            "Threads": 1, "Hash": args.hash_mb, "Move Overhead": args.move_overhead_ms,
        })
        for board, expected in positions:
            ref = run_search(engine, board, reference)
            first_match = next((p for p in ref["iterations"] if p["move"] in expected), None)
            if first_match is not None:
                matches.append(first_match)
            row = {"fen": board.fen(), "expected": sorted(expected), "reference": ref,
                   "first_match": first_match, "searches": []}
            for (kind, cap), limit in budgets.items():
                result = run_search(engine, board, limit)
                iterations = result["iterations"]
                last = iterations[-1] if iterations else None
                correct = result["bestmove"] in expected
                depth = last["depth"] if last else 0
                total = totals[kind, cap]
                total["correct"] += correct
                total["last_pv_correct"] += last is not None and last["move"] in expected
                total["miss_fixed_at_next_reference_depth"] += not correct and any(
                    p["depth"] == depth + 1 and p["move"] in expected for p in ref["iterations"]
                )
                row["searches"].append({"kind": kind, "cap": cap, **result})
            if output is not None:
                print(json.dumps(row), file=output)
    print(f"Positions: {len(positions)}")
    print(f"Reference found an expected move: {len(matches)}")
    if matches:
        print(f"Median depth of first match: {median(match['depth'] for match in matches):g}")
        nodes = [match["nodes"] for match in matches if match["nodes"] is not None]
        if nodes:
            print(f"Median nodes of first match: {median(nodes):g}")
    for (kind, cap), total in totals.items():
        print(kind, cap, dict(total))


if __name__ == "__main__":
    main()
