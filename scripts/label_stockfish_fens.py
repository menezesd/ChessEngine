#!/usr/bin/env python3
"""Label sampled FEN/result rows with Stockfish centipawn evals."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import queue
import threading
import time
from pathlib import Path

import chess


if __package__:
    from .uci_client import UCIClient
else:
    from uci_client import UCIClient


class Stockfish(UCIClient):
    def __init__(self, path: str, hash_mb: int):
        super().__init__(path, ["Threads value 1", f"Hash value {hash_mb}"])

    def evaluate(self, fen: str, depth: int) -> int | None:
        score, _ = self.score_position(fen, f"go depth {depth}", timeout=90.0)
        if score is None:
            return None
        return score if chess.Board(fen).turn == chess.WHITE else -score


def parse_rows(path: Path, limit: int) -> list[tuple[str, str]]:
    rows: list[tuple[str, str]] = []
    with path.open() as f:
        for line in f:
            line = line.strip()
            if not line or "|" not in line:
                continue
            fen, result = [part.strip() for part in line.rsplit("|", 1)]
            rows.append((fen, result))
            if limit and len(rows) >= limit:
                break
    return rows


def label_chunk(
    rows: list[tuple[str, str]],
    stockfish: str,
    depth: int,
    hash_mb: int,
    progress: queue.Queue[int],
) -> list[tuple[str, int, str]]:
    engine = Stockfish(stockfish, hash_mb)
    labelled: list[tuple[str, int, str]] = []
    try:
        for fen, result in rows:
            score = engine.evaluate(fen, depth)
            if score is not None:
                labelled.append((fen, score, result))
            progress.put(1)
    finally:
        engine.quit()
    return labelled


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--stockfish", default="/usr/local/bin/stockfish")
    parser.add_argument("--depth", type=int, default=8)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--hash-mb", type=int, default=64)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--clip-cp", type=int, default=2000)
    args = parser.parse_args()

    rows = parse_rows(args.input, args.limit)
    chunks = [rows[i:: args.workers] for i in range(args.workers)]
    progress: queue.Queue[int] = queue.Queue()
    done = 0
    started = time.time()

    def report() -> None:
        nonlocal done
        while done < len(rows):
            try:
                done += progress.get(timeout=1)
            except queue.Empty:
                pass
            elapsed = max(time.time() - started, 1e-6)
            print(
                f"\rlabelled {done:,}/{len(rows):,} "
                f"({done / elapsed:.1f} pos/s)",
                end="",
                flush=True,
            )

    reporter = threading.Thread(target=report, daemon=True)
    reporter.start()

    labelled: list[tuple[str, int, str]] = []
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [
            pool.submit(label_chunk, chunk, args.stockfish, args.depth, args.hash_mb, progress)
            for chunk in chunks
            if chunk
        ]
        for future in as_completed(futures):
            labelled.extend(future.result())

    reporter.join(timeout=1)
    print()

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w") as out:
        for fen, score, result in labelled:
            score = max(-args.clip_cp, min(args.clip_cp, score))
            out.write(f"{fen} | {score} | {result}\n")

    print(f"labelled {len(labelled):,} positions -> {args.output}")


if __name__ == "__main__":
    main()
