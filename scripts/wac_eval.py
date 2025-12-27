#!/usr/bin/env python3
"""
Quick WAC dataset evaluator.

Reads an EPD file with `bm` fields, asks the engine for a move, and reports
how often the engine finds the expected best move.
"""

import argparse
import collections
import queue
import subprocess
import sys
import time
from pathlib import Path

import chess


class UCIEngine:
    def __init__(self, path: str, name: str, threads: int = 1):
        self.name = name
        self.threads = threads
        self.proc = subprocess.Popen(
            [path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        self.lines: queue.Queue[str] = queue.Queue()
        self._reader = collections.deque(maxlen=50)
        self._reader_thread = subprocess.Popen
        self._start_reader()
        self._handshake(threads)

    def _start_reader(self) -> None:
        assert self.proc.stdout is not None

        def loop():
            for line in self.proc.stdout:
                clean = line.rstrip("\n")
                self._reader.append(clean)
                self.lines.put(clean)

        import threading

        t = threading.Thread(target=loop, daemon=True)
        t.start()
        self._reader_thread = t

    def _send(self, cmd: str) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def _drain_until(self, token: str, timeout: float) -> None:
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if token in line:
                return
        recent = "\n".join(self._reader)
        raise RuntimeError(f"{self.name}: timeout waiting for {token}\nRecent:\n{recent}")

    def _handshake(self, threads: int = 1) -> None:
        self._send("uci")
        self._drain_until("uciok", timeout=5.0)
        if threads > 1:
            self._send(f"setoption name Threads value {threads}")
        self._send("isready")
        self._drain_until("readyok", timeout=5.0)

    def bestmove_for_fen(self, fen: str, movetime_ms: int, timeout: float) -> str:
        self._send("ucinewgame")
        self._send(f"position fen {fen}")
        self._send(f"go movetime {movetime_ms}")
        return self._read_bestmove(timeout)

    def _read_bestmove(self, timeout: float) -> str:
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                if self.proc.poll() is not None:
                    recent = "\n".join(self._reader)
                    raise RuntimeError(
                        f"{self.name}: exited with {self.proc.returncode}\nRecent output:\n{recent}"
                    )
                continue
            if line.startswith("bestmove "):
                return line.split()[1]
        recent = "\n".join(self._reader)
        raise RuntimeError(
            f"{self.name}: timeout waiting for bestmove\nRecent output:\n{recent}"
        )

    def quit(self) -> None:
        if self.proc.poll() is None:
            self._send("quit")
            try:
                self.proc.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def parse_epd_line(line: str) -> tuple[str, list[str], str]:
    """Return (FEN, best_moves, id)."""
    tokens = line.strip().split()
    if len(tokens) < 6 or "bm" not in tokens:
        raise ValueError(f"Malformed EPD: {line}")
    fen = " ".join(tokens[:4] + ["0", "1"])
    bm_idx = tokens.index("bm")
    best_moves = []
    for mv in tokens[bm_idx + 1 :]:
        if mv.endswith(";"):
            mv = mv.rstrip(";")
        if mv == "id":
            break
        best_moves.append(mv)
    if "id" in tokens:
        id_idx = tokens.index("id")
        epd_id = tokens[id_idx + 1].strip('";')
    else:
        epd_id = "unknown"
    return fen, best_moves, epd_id


def uci_to_san(fen: str, uci_move: str) -> str:
    """Convert UCI move to SAN notation."""
    try:
        board = chess.Board(fen)
        move = chess.Move.from_uci(uci_move)
        return board.san(move)
    except Exception:
        return uci_move  # Return original if conversion fails


def run_wac(engine_path: str, epd_path: Path, limit: int, movetime_ms: int, threads: int = 1) -> None:
    engine = UCIEngine(engine_path, "engine", threads=threads)
    total = 0
    correct = 0
    mismatches = []

    with epd_path.open() as f:
        for line in f:
            if not line.strip():
                continue
            if total >= limit:
                break
            fen, bms, epd_id = parse_epd_line(line)
            total += 1
            best_uci = engine.bestmove_for_fen(fen, movetime_ms, timeout=10.0)
            best_san = uci_to_san(fen, best_uci)
            if best_san in bms or best_uci in bms:
                correct += 1
            else:
                mismatches.append((epd_id, best_san, bms[0]))

    engine.quit()
    print(f"WAC results ({epd_path.name}, limit={limit}, movetime={movetime_ms}ms, threads={threads}): {correct}/{total}")
    if mismatches:
        print("Sample misses (id, got, expected):")
        for epd_id, got, exp in mismatches[:10]:
            print(f"  {epd_id}: {got} (expected {exp})")


def main() -> None:
    parser = argparse.ArgumentParser(description="Evaluate engine on WAC EPD file.")
    parser.add_argument("--engine", default="./target/release/chess_engine", help="Path to UCI engine binary")
    parser.add_argument("--epd", default="tests/data/wac_batch_50.epd", help="EPD file with bm fields")
    parser.add_argument("--limit", type=int, default=20, help="Number of positions to evaluate")
    parser.add_argument("--movetime-ms", type=int, default=500, help="Movetime per position (ms)")
    parser.add_argument("--threads", type=int, default=1, help="Number of search threads")
    args = parser.parse_args()

    run_wac(args.engine, Path(args.epd), args.limit, args.movetime_ms, args.threads)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(1)
