#!/usr/bin/env python3
"""
Generate NNUE training data using Stockfish for labeling.

This produces higher quality training data than self-play by using
Stockfish's strong evaluation to label positions.

Usage:
    python stockfish_label.py --positions 100000 --output data/sf_labeled.txt
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
from pathlib import Path
from typing import List, Tuple, Optional
from concurrent.futures import ThreadPoolExecutor, as_completed

try:
    import chess
    import chess.pgn
except ImportError:
    print("Error: python-chess required. Install with: pip install chess")
    sys.exit(1)


class StockfishEngine:
    """Wrapper for Stockfish UCI engine."""

    def __init__(self, path: str = "stockfish", threads: int = 1, hash_mb: int = 128):
        self.path = path
        self.threads = threads
        self.hash_mb = hash_mb
        self.proc: Optional[subprocess.Popen] = None
        self.lines: queue.Queue = queue.Queue()

    def start(self):
        self.proc = subprocess.Popen(
            [self.path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )

        def reader():
            for line in self.proc.stdout:
                self.lines.put(line.strip())

        threading.Thread(target=reader, daemon=True).start()

        self._send("uci")
        self._wait_for("uciok")
        self._send(f"setoption name Threads value {self.threads}")
        self._send(f"setoption name Hash value {self.hash_mb}")
        self._send("isready")
        self._wait_for("readyok")

    def _send(self, cmd: str):
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def _wait_for(self, target: str, timeout: float = 10.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
                if line.startswith(target):
                    return line
            except queue.Empty:
                continue
        raise RuntimeError(f"Timeout waiting for {target}")

    def evaluate(self, fen: str, depth: int = 12) -> Tuple[int, bool]:
        """Evaluate position. Returns (score_cp, is_mate)."""
        self._send("ucinewgame")
        self._send(f"position fen {fen}")
        self._send(f"go depth {depth}")

        score = 0
        is_mate = False

        deadline = time.time() + 60.0
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
                if "score cp" in line:
                    match = re.search(r"score cp (-?\d+)", line)
                    if match:
                        score = int(match.group(1))
                        is_mate = False
                elif "score mate" in line:
                    match = re.search(r"score mate (-?\d+)", line)
                    if match:
                        mate_in = int(match.group(1))
                        score = 30000 - abs(mate_in) * 100
                        if mate_in < 0:
                            score = -score
                        is_mate = True
                elif line.startswith("bestmove"):
                    break
            except queue.Empty:
                continue

        return score, is_mate

    def quit(self):
        if self.proc and self.proc.poll() is None:
            self._send("quit")
            try:
                self.proc.wait(timeout=2.0)
            except:
                self.proc.kill()


def generate_random_position(max_moves: int = 40) -> Optional[chess.Board]:
    """Generate a random legal position by playing random moves."""
    board = chess.Board()
    num_moves = random.randint(8, max_moves)

    for _ in range(num_moves):
        legal_moves = list(board.legal_moves)
        if not legal_moves:
            break
        move = random.choice(legal_moves)
        board.push(move)

        if board.is_game_over():
            break

    if board.is_game_over():
        return None

    return board


def generate_opening_position(openings: List[List[str]], extra_moves: int = 10) -> Optional[chess.Board]:
    """Generate a position from an opening line plus some random moves."""
    board = chess.Board()

    if openings:
        opening = random.choice(openings)
        for move_san in opening:
            try:
                move = board.parse_san(move_san)
                board.push(move)
            except:
                break

    # Add some random moves
    for _ in range(random.randint(0, extra_moves)):
        legal_moves = list(board.legal_moves)
        if not legal_moves:
            break
        move = random.choice(legal_moves)
        board.push(move)

        if board.is_game_over():
            return None

    return board


# Common chess openings (in SAN notation)
OPENINGS = [
    # Italian Game
    ["e4", "e5", "Nf3", "Nc6", "Bc4"],
    ["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5"],
    ["e4", "e5", "Nf3", "Nc6", "Bc4", "Nf6"],
    # Ruy Lopez
    ["e4", "e5", "Nf3", "Nc6", "Bb5"],
    ["e4", "e5", "Nf3", "Nc6", "Bb5", "a6"],
    ["e4", "e5", "Nf3", "Nc6", "Bb5", "Nf6"],
    # Sicilian
    ["e4", "c5"],
    ["e4", "c5", "Nf3", "d6"],
    ["e4", "c5", "Nf3", "Nc6"],
    ["e4", "c5", "Nf3", "e6"],
    # French
    ["e4", "e6"],
    ["e4", "e6", "d4", "d5"],
    # Caro-Kann
    ["e4", "c6"],
    ["e4", "c6", "d4", "d5"],
    # Queen's Gambit
    ["d4", "d5", "c4"],
    ["d4", "d5", "c4", "e6"],
    ["d4", "d5", "c4", "c6"],
    # King's Indian
    ["d4", "Nf6", "c4", "g6"],
    # Nimzo-Indian
    ["d4", "Nf6", "c4", "e6", "Nc3", "Bb4"],
    # English
    ["c4"],
    ["c4", "e5"],
    ["c4", "Nf6"],
    # Scotch
    ["e4", "e5", "Nf3", "Nc6", "d4"],
    # Vienna
    ["e4", "e5", "Nc3"],
    # Pirc
    ["e4", "d6", "d4", "Nf6"],
    # Alekhine
    ["e4", "Nf6"],
    # Scandinavian
    ["e4", "d5"],
    # London System
    ["d4", "d5", "Bf4"],
    ["d4", "Nf6", "Bf4"],
]


def worker_generate_and_label(
    worker_id: int,
    num_positions: int,
    stockfish_path: str,
    depth: int,
    openings: List[List[str]],
    progress_queue: queue.Queue
) -> List[Tuple[str, int]]:
    """Worker function to generate and label positions."""

    sf = StockfishEngine(stockfish_path)
    sf.start()

    positions = []

    try:
        for i in range(num_positions):
            # Mix of opening-based and random positions
            if random.random() < 0.7:
                board = generate_opening_position(openings, extra_moves=random.randint(5, 25))
            else:
                board = generate_random_position(max_moves=random.randint(20, 60))

            if board is None:
                continue

            fen = board.fen()

            # Get Stockfish evaluation
            score, is_mate = sf.evaluate(fen, depth=depth)

            # Skip extreme evaluations (likely won/lost positions)
            if abs(score) > 5000 and not is_mate:
                continue

            # Store score from white's perspective
            if not board.turn:  # Black to move
                score = -score

            positions.append((fen, score))

            if (i + 1) % 100 == 0:
                progress_queue.put(100)

    finally:
        sf.quit()

    return positions


def main():
    parser = argparse.ArgumentParser(description="Generate Stockfish-labeled training data")
    parser.add_argument("--positions", type=int, default=100000,
                        help="Number of positions to generate")
    parser.add_argument("--output", type=str, default="data/sf_labeled.txt",
                        help="Output file path")
    parser.add_argument("--stockfish", type=str, default="/opt/local/bin/stockfish",
                        help="Path to Stockfish executable")
    parser.add_argument("--depth", type=int, default=12,
                        help="Stockfish search depth")
    parser.add_argument("--workers", type=int, default=4,
                        help="Number of parallel workers")
    parser.add_argument("--append", action="store_true",
                        help="Append to existing file")
    args = parser.parse_args()

    # Check Stockfish exists
    if not os.path.exists(args.stockfish):
        # Try to find it
        result = subprocess.run(["which", "stockfish"], capture_output=True, text=True)
        if result.returncode == 0:
            args.stockfish = result.stdout.strip()
        else:
            print(f"Error: Stockfish not found at {args.stockfish}")
            sys.exit(1)

    print(f"Stockfish path: {args.stockfish}")
    print(f"Target positions: {args.positions}")
    print(f"Search depth: {args.depth}")
    print(f"Workers: {args.workers}")
    print(f"Output: {args.output}")
    print()

    # Create output directory
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)

    # Progress tracking
    progress_queue = queue.Queue()
    total_done = 0

    # Distribute work
    positions_per_worker = args.positions // args.workers
    remainder = args.positions % args.workers

    all_positions = []
    start_time = time.time()

    def progress_reporter():
        nonlocal total_done
        while True:
            try:
                count = progress_queue.get(timeout=1.0)
                if count is None:
                    break
                total_done += count
                elapsed = time.time() - start_time
                rate = total_done / elapsed if elapsed > 0 else 0
                eta = (args.positions - total_done) / rate if rate > 0 else 0
                print(f"\rProgress: {total_done}/{args.positions} ({100*total_done/args.positions:.1f}%) "
                      f"Rate: {rate:.1f} pos/s, ETA: {eta:.0f}s", end="", flush=True)
            except queue.Empty:
                continue

    reporter = threading.Thread(target=progress_reporter, daemon=True)
    reporter.start()

    print("Generating and labeling positions...")

    with ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = []
        for i in range(args.workers):
            worker_positions = positions_per_worker + (1 if i < remainder else 0)
            if worker_positions > 0:
                futures.append(executor.submit(
                    worker_generate_and_label,
                    i, worker_positions, args.stockfish, args.depth, OPENINGS, progress_queue
                ))

        for future in as_completed(futures):
            all_positions.extend(future.result())

    progress_queue.put(None)  # Stop reporter
    print()

    # Shuffle and write
    random.shuffle(all_positions)

    mode = "a" if args.append else "w"
    with open(args.output, mode) as f:
        for fen, score in all_positions:
            # Format: FEN | score | result (0.5 since we don't have game outcome)
            f.write(f"{fen} | {score} | 0.5\n")

    elapsed = time.time() - start_time
    print(f"\nDone! Generated {len(all_positions)} labeled positions in {elapsed:.1f}s")
    print(f"Output saved to: {args.output}")


if __name__ == "__main__":
    main()
