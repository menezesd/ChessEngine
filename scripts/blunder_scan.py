#!/usr/bin/env python3
import collections
import os
import queue
import subprocess
import sys
import threading
import time


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
        self.recent = collections.deque(maxlen=50)
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

    def position(self, moves):
        if moves:
            self._send("position startpos moves " + " ".join(moves))
        else:
            self._send("position startpos")

    def go(self, movetime_ms):
        self._send(f"go movetime {movetime_ms}")
        return self._read_score_and_bestmove(timeout=max(5.0, movetime_ms / 1000 + 2))

    def go_searchmoves(self, movetime_ms, move):
        self._send(f"go movetime {movetime_ms} searchmoves {move}")
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


def parse_moves():
    if len(sys.argv) > 1:
        return sys.argv[1:]
    text = sys.stdin.read().strip()
    if not text:
        return []
    return text.split()


def main():
    moves = parse_moves()
    if not moves:
        print("usage: blunder_scan.py <move1> <move2> ...", file=sys.stderr)
        print("or: echo \"e2e4 e7e5 ...\" | blunder_scan.py", file=sys.stderr)
        return 2

    stockfish = os.environ.get("STOCKFISH", "stockfish")
    movetime_ms = int(os.environ.get("MOVETIME_MS", "500"))
    blunder_cp = int(os.environ.get("BLUNDER_CP", "300"))

    sf = UCIEngine(stockfish, "stockfish")
    sf.setoption("MultiPV", "2")
    sf._send("isready")
    sf._drain_until("readyok", timeout=5.0)

    try:
        played = []
        print(f"movetime_ms={movetime_ms} blunder_cp={blunder_cp}")
        for ply, move in enumerate(moves, start=1):
            sf.position(played)
            best_score, best_move = sf.go(movetime_ms)
            if best_score is None:
                best_score = 0
            sf.position(played)
            move_score, _ = sf.go_searchmoves(movetime_ms, move)
            if move_score is None:
                move_score = 0
            played.append(move)

            score_before = best_score
            score_after = -move_score
            delta = score_after - score_before
            tag = "OK"
            if delta <= -blunder_cp:
                tag = "BLUNDER"

            side = "white" if ply % 2 == 1 else "black"
            print(
                f"ply {ply:3d} {side:5s} {move} | best {best_move} "
                f"eval {score_before:+5d} -> {score_after:+5d} "
                f"delta {delta:+5d} {tag}"
            )
    finally:
        sf.quit()

    return 0


if __name__ == "__main__":
    sys.exit(main())
