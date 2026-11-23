#!/usr/bin/env python3
"""Label or relabel FEN/result rows with Stockfish centipawn scores."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import math
import queue
import tempfile
import threading
import time
from pathlib import Path
from typing import NamedTuple

import chess


if __package__:
    from .uci_client import UCIClient
else:
    from uci_client import UCIClient


class Stockfish(UCIClient):
    def __init__(self, path: str, hash_mb: int):
        super().__init__(path, ["Threads value 1", f"Hash value {hash_mb}"])

    def evaluate(self, fen: str, depth: int) -> tuple[int, bool] | None:
        score, is_mate = self.score_position(fen, f"go depth {depth}", timeout=90.0)
        if score is None:
            return None
        if not chess.Board(fen).turn == chess.WHITE:
            score = -score
        return score, is_mate


class LabelRow(NamedTuple):
    fen: str
    result: str
    score: int | float | None = None


class LabelSearch(NamedTuple):
    depth: int
    clip_cp: int
    screen_depth: int | None
    screen_threshold: int


def parse_rows(path: Path, limit: int) -> list[LabelRow]:
    rows: list[LabelRow] = []
    with path.open() as f:
        for line_number, line in enumerate(f, 1):
            line = line.strip()
            if not line or "|" not in line:
                continue
            fields = [part.strip() for part in line.split("|")]
            if len(fields) == 2:
                fen, result = fields
                rows.append(LabelRow(fen, result))
            elif len(fields) == 3:
                fen, score, result = fields
                try:
                    value = float(score)
                    if not math.isfinite(value):
                        raise ValueError("score must be finite")
                    rows.append(LabelRow(fen, result, int(value) if value.is_integer() else value))
                except ValueError as error:
                    raise ValueError(f"{path}:{line_number}: invalid centipawn score {score!r}") from error
            else:
                raise ValueError(f"{path}:{line_number}: expected a FEN, optional score, and result")
            if limit and len(rows) >= limit:
                break
    return rows


def label_chunk(
    rows: list[LabelRow],
    stockfish: str,
    hash_mb: int,
    progress: queue.Queue[int],
    search: LabelSearch,
) -> list[LabelRow]:
    engine = Stockfish(stockfish, hash_mb)
    labelled: list[LabelRow] = []
    try:
        for row in rows:
            updated = label_row(engine, row, search)
            if updated.score is not None:
                labelled.append(updated)
            progress.put(1)
    finally:
        engine.quit()
    return labelled


def label_row(engine: Stockfish, row: LabelRow, search: LabelSearch) -> LabelRow:
    if search.screen_depth is not None and row.score is not None:
        screened = engine.evaluate(row.fen, search.screen_depth)
        if screened is not None:
            score, is_mate = screened
            if not is_mate and abs(score - row.score) < search.screen_threshold:
                return row

    evaluated = engine.evaluate(row.fen, search.depth)
    if evaluated is None:
        return row
    score, is_mate = evaluated
    limit = 32_000 - 3 if is_mate else search.clip_cp
    return LabelRow(row.fen, row.result, max(-limit, min(limit, score)))


def write_labels(path: Path, rows: list[LabelRow]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         delete=False) as output:
            temporary = Path(output.name)
            for row in rows:
                output.write(f"{row.fen} | {row.score} | {row.result}\n")
        temporary.replace(path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


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
    parser.add_argument("--screen-depth", type=int, help="Check existing labels at this depth first")
    parser.add_argument("--screen-threshold", type=int, default=40,
                        help="Disagreement in cp that triggers the deeper search")
    args = parser.parse_args()

    if args.workers < 1:
        parser.error("--workers must be positive")
    if args.screen_depth is not None and not 1 <= args.screen_depth < args.depth:
        parser.error("--screen-depth must be positive and smaller than --depth")
    search = LabelSearch(args.depth, args.clip_cp, args.screen_depth, args.screen_threshold)
    rows = parse_rows(args.input, args.limit)
    chunk_size = max(1, (len(rows) + args.workers - 1) // args.workers)
    chunks = [rows[start:start + chunk_size] for start in range(0, len(rows), chunk_size)]
    progress: queue.Queue[int] = queue.Queue()
    done = 0
    started = time.monotonic()

    def report() -> None:
        nonlocal done
        while done < len(rows):
            try:
                done += progress.get(timeout=1)
            except queue.Empty:
                pass
            elapsed = max(time.monotonic() - started, 1e-6)
            print(
                f"\rlabelled {done:,}/{len(rows):,} "
                f"({done / elapsed:.1f} pos/s)",
                end="",
                flush=True,
            )

    reporter = threading.Thread(target=report, daemon=True)
    reporter.start()

    labelled: list[LabelRow] = []
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [
            pool.submit(label_chunk, chunk, args.stockfish, args.hash_mb, progress, search)
            for chunk in chunks
        ]
        for future in futures:
            labelled.extend(future.result())

    reporter.join(timeout=1)
    print()

    write_labels(args.output, labelled)

    print(f"labelled {len(labelled):,} positions -> {args.output}")


if __name__ == "__main__":
    main()
