#!/usr/bin/env python3
import argparse
import collections
import queue
import re
import subprocess
import sys
import threading
import time

try:
    import chess
except Exception as exc:
    print(f"python-chess is required: {exc}", file=sys.stderr)
    sys.exit(2)

EPD_ID_RE = re.compile(r"id\s+\"([^\"]+)\"")


class UCIEngine:
    def __init__(self, path, name):
        self.path = path
        self.name = name
        self.proc = subprocess.Popen(
            [path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        self.lines = queue.Queue()
        self.recent = collections.deque(maxlen=100)
        self.reader = threading.Thread(target=self._read_stdout, daemon=True)
        self.reader.start()
        self._handshake()

    def _read_stdout(self):
        assert self.proc.stdout is not None
        for line in self.proc.stdout:
            clean = line.rstrip("\n")
            self.recent.append(clean)
            self.lines.put(clean)

    def _send(self, cmd):
        assert self.proc.stdin is not None
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def _handshake(self):
        self._send("uci")
        self._drain_until("uciok", timeout=5.0)
        self._send("isready")
        self._drain_until("readyok", timeout=5.0)

    def _drain_until(self, token, timeout):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if token in line:
                return True
        raise RuntimeError(f"{self.name}: timeout waiting for {token}")

    def setoption(self, name, value):
        self._send(f"setoption name {name} value {value}")

    def position_fen(self, fen):
        self._send(f"position fen {fen}")

    def go(self, movetime_ms):
        self._send(f"go movetime {movetime_ms}")
        return self._read_score_and_bestmove(timeout=max(5.0, movetime_ms / 1000 + 2))

    def _read_score_and_bestmove(self, timeout):
        deadline = time.time() + timeout
        last_score = None
        last_best = None
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                if self.proc.poll() is not None:
                    recent = "\n".join(self.recent)
                    raise RuntimeError(
                        f"{self.name}: exited with {self.proc.returncode}\n"
                        f"{self.name} recent output:\n{recent}"
                    )
                continue
            if line.startswith("info ") and "score " in line:
                parts = line.split()
                if "score" in parts:
                    idx = parts.index("score")
                    if idx + 2 < len(parts):
                        kind = parts[idx + 1]
                        val = parts[idx + 2]
                        if kind == "cp":
                            try:
                                last_score = int(val)
                            except ValueError:
                                pass
                        elif kind == "mate":
                            try:
                                mate = int(val)
                                last_score = 100000 if mate > 0 else -100000
                            except ValueError:
                                pass
            if line.startswith("bestmove "):
                last_best = line.split()[1]
                return last_score, last_best
        recent = "\n".join(self.recent)
        raise RuntimeError(
            f"{self.name}: timeout waiting for bestmove\n{self.name} recent output:\n{recent}"
        )

    def quit(self):
        if self.proc.poll() is None:
            self._send("quit")
            try:
                self.proc.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def normalize_fen(fen):
    parts = fen.split()
    if len(parts) == 4:
        return fen + " 0 1"
    if len(parts) == 5:
        return fen + " 1"
    return fen


def parse_epd_line(line):
    trimmed = line.strip()
    if not trimmed or trimmed.startswith("#"):
        return None
    if " bm " not in trimmed:
        raise ValueError(f"EPD missing bm: {trimmed}")
    fen_part, rest = trimmed.split(" bm ", 1)
    bm_raw = rest.split(";", 1)[0].strip()
    bm_moves = [m for m in bm_raw.split() if m]
    match = EPD_ID_RE.search(trimmed)
    epd_id = match.group(1) if match else "?"
    return normalize_fen(fen_part.strip()), bm_moves, epd_id


def bm_to_uci(board, bm_moves):
    uci_moves = []
    for san in bm_moves:
        try:
            move = board.parse_san(san)
        except ValueError:
            return None
        uci_moves.append(move.uci())
    return uci_moves


def uci_to_san(board, uci_move):
    try:
        move = board.parse_uci(uci_move)
    except ValueError:
        return None
    if move not in board.legal_moves:
        return None
    return board.san(move)


def load_epd(path):
    with open(path, "r", encoding="utf-8") as handle:
        for raw in handle:
            parsed = parse_epd_line(raw)
            if parsed is not None:
                yield parsed


def main():
    parser = argparse.ArgumentParser(description="Run a tactics batch against the engine.")
    parser.add_argument("--engine", required=True, help="Path to UCI engine binary")
    parser.add_argument("--epd", required=True, help="Path to EPD/EPD-like file")
    parser.add_argument("--movetime", type=int, default=5000, help="Time per position in ms")
    parser.add_argument("--limit", type=int, default=0, help="Stop after N positions")
    args = parser.parse_args()

    engine = UCIEngine(args.engine, "engine")
    total = 0
    correct = 0

    try:
        for fen, bm_moves, epd_id in load_epd(args.epd):
            total += 1
            board = chess.Board(fen)
            expected_uci = bm_to_uci(board, bm_moves)
            if expected_uci is None:
                print(f"{epd_id}: unable to parse bm moves: {' '.join(bm_moves)}")
                continue
            engine.position_fen(fen)
            score, best = engine.go(args.movetime)
            best_san = uci_to_san(board, best)
            ok = best in expected_uci
            tag = "OK" if ok else "MISS"
            if ok:
                correct += 1
            expected_san = []
            for uci in expected_uci:
                san = uci_to_san(board, uci)
                expected_san.append(san or uci)
            score_str = "?" if score is None else f"{score:+d}"
            print(
                f"{epd_id} {tag} | best {best}"
                f" ({best_san or '?'}) | eval {score_str} | bm {', '.join(expected_san)}"
            )
            if args.limit and total >= args.limit:
                break
    finally:
        engine.quit()

    if total:
        print(f"\nResult: {correct}/{total} ({correct / total:.1%})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
