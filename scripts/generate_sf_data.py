#!/usr/bin/env python3
"""Generate SF-labeled training data from PGN files.

Extracts positions from games, labels them with Stockfish, outputs in
the standard training format: FEN | eval_cp_white | result_white

Usage:
    python3 scripts/generate_sf_data.py \
        --pgn /path/to/games.pgn \
        --output data/new_labeled.txt \
        --positions 1000000 \
        --depth 12 \
        --workers 4
"""
import argparse
import os
import queue
import random
import re
import subprocess
import sys
import threading
import time
from concurrent.futures import ProcessPoolExecutor, as_completed

import chess
import chess.pgn


def sf_worker(positions_batch, sf_path, depth):
    """Label a batch of positions with Stockfish."""
    proc = subprocess.Popen(
        [sf_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        bufsize=1,
    )
    lines_q = queue.Queue()
    threading.Thread(
        target=lambda: [lines_q.put(l.strip()) for l in proc.stdout], daemon=True
    ).start()

    def send(cmd):
        proc.stdin.write(cmd + "\n")
        proc.stdin.flush()

    def wait_for(tok, timeout=10.0):
        dl = time.time() + timeout
        while time.time() < dl:
            try:
                line = lines_q.get(timeout=0.1)
                if line.startswith(tok):
                    return line
            except queue.Empty:
                continue
        return None

    send("uci")
    wait_for("uciok")
    send("setoption name Threads value 1")
    send("setoption name Hash value 64")
    send("isready")
    wait_for("readyok")

    results = []
    for fen, game_result in positions_batch:
        send(f"position fen {fen}")
        send(f"go depth {depth}")

        score = None
        is_mate = False
        dl = time.time() + 60.0
        while time.time() < dl:
            try:
                line = lines_q.get(timeout=0.1)
                m = re.search(r"score cp (-?\d+)", line)
                if m:
                    score = int(m.group(1))
                    is_mate = False
                m2 = re.search(r"score mate (-?\d+)", line)
                if m2:
                    mate_in = int(m2.group(1))
                    score = 30000 - abs(mate_in) * 100
                    if mate_in < 0:
                        score = -score
                    is_mate = True
                if line.startswith("bestmove"):
                    break
            except queue.Empty:
                continue

        if score is not None:
            # Convert STM eval to WHITE perspective
            board = chess.Board(fen)
            if not board.turn:  # Black to move
                eval_white = -score
            else:
                eval_white = score
            results.append(f"{fen} | {eval_white} | {game_result}")

    send("quit")
    try:
        proc.wait(timeout=2)
    except:
        proc.kill()
    return results


def extract_positions_from_pgn(pgn_path, max_positions, skip_first_n=8, sample_rate=0.3):
    """Extract positions from a PGN file."""
    positions = []
    games = 0

    with open(pgn_path) as f:
        while len(positions) < max_positions:
            game = chess.pgn.read_game(f)
            if game is None:
                break
            games += 1

            # Get game result
            result_str = game.headers.get("Result", "*")
            if result_str == "1-0":
                game_result = 1.0
            elif result_str == "0-1":
                game_result = 0.0
            elif result_str == "1/2-1/2":
                game_result = 0.5
            else:
                continue

            board = game.board()
            ply = 0
            for move in game.mainline_moves():
                board.push(move)
                ply += 1

                # Skip early opening moves
                if ply < skip_first_n * 2:
                    continue

                # Skip positions in check
                if board.is_check():
                    continue

                # Skip game-over positions
                if board.is_game_over():
                    continue

                # Random sampling to get diversity
                if random.random() > sample_rate:
                    continue

                positions.append((board.fen(), game_result))

                if len(positions) >= max_positions:
                    break

            if games % 1000 == 0:
                print(
                    f"  Processed {games} games, {len(positions)} positions extracted",
                    flush=True,
                )

    print(f"Extracted {len(positions)} positions from {games} games", flush=True)
    return positions


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pgn", required=True, nargs="+", help="PGN file(s)")
    parser.add_argument("--output", required=True, help="Output training data file")
    parser.add_argument(
        "--positions", type=int, default=1000000, help="Max positions to extract"
    )
    parser.add_argument("--depth", type=int, default=12, help="SF search depth")
    parser.add_argument("--workers", type=int, default=4, help="Parallel SF workers")
    parser.add_argument(
        "--sf-path", default="stockfish", help="Path to Stockfish binary"
    )
    parser.add_argument(
        "--batch-size", type=int, default=500, help="Positions per SF worker batch"
    )
    parser.add_argument(
        "--skip-first",
        type=int,
        default=8,
        help="Opening moves to skip before sampling positions",
    )
    parser.add_argument(
        "--sample-rate",
        type=float,
        default=0.3,
        help="Probability of keeping each eligible position",
    )
    parser.add_argument(
        "--allocation",
        choices=["equal", "size"],
        default="equal",
        help="How to split the requested position cap across PGN files",
    )
    parser.add_argument("--seed", type=int, default=42, help="Random sampling seed")
    parser.add_argument(
        "--extract-only",
        action="store_true",
        help="Only extract and count sampled positions; do not label or write output",
    )
    args = parser.parse_args()
    random.seed(args.seed)

    # Extract positions from PGNs
    print("Extracting positions from PGNs...")
    all_positions = []
    if args.allocation == "size":
        sizes = [max(os.path.getsize(pgn), 1) for pgn in args.pgn]
        total_size = sum(sizes)
        per_file_caps = [
            max(1, int(args.positions * size / total_size) + 1) for size in sizes
        ]
    else:
        per_file = args.positions // len(args.pgn) + 1
        per_file_caps = [per_file for _ in args.pgn]

    for pgn, cap in zip(args.pgn, per_file_caps):
        print(f"  {pgn}")
        positions = extract_positions_from_pgn(
            pgn,
            cap,
            skip_first_n=args.skip_first,
            sample_rate=args.sample_rate,
        )
        all_positions.extend(positions)

    random.shuffle(all_positions)
    all_positions = all_positions[: args.positions]
    print(f"Total: {len(all_positions)} positions to label")
    if args.extract_only:
        return

    # Label with Stockfish in parallel
    print(f"Labeling with SF depth {args.depth} using {args.workers} workers...")
    batches = []
    for i in range(0, len(all_positions), args.batch_size):
        batches.append(all_positions[i : i + args.batch_size])

    done = 0
    with open(args.output, "w") as out:
        with ProcessPoolExecutor(max_workers=args.workers) as executor:
            futures = {
                executor.submit(sf_worker, batch, args.sf_path, args.depth): batch
                for batch in batches
            }
            for future in as_completed(futures):
                results = future.result()
                for line in results:
                    out.write(line + "\n")
                done += len(results)
                if done % 10000 < args.batch_size:
                    out.flush()
                    print(f"  Labeled {done}/{len(all_positions)} positions", flush=True)

    print(f"Done! Wrote {done} labeled positions to {args.output}")


if __name__ == "__main__":
    main()
