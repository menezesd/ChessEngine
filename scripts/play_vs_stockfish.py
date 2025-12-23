#!/usr/bin/env python3
import os
import queue
import collections
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

    def setoption(self, name, value):
        self._send(f"setoption name {name} value {value}")

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

    def bestmove(self, moves, movetime_ms=None, time_control=None, timeout=10.0):
        position = "position startpos"
        if moves:
            position += " moves " + " ".join(moves)
        self._send(position)
        if time_control:
            self._send(time_control.go_command())
        elif movetime_ms is not None:
            self._send(f"go movetime {movetime_ms}")
        else:
            self._send("go")
        return self._read_bestmove(timeout=timeout)

    def _read_bestmove(self, timeout):
        deadline = time.time() + timeout
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
            if line.startswith("bestmove "):
                return line.split()[1]
        recent = "\n".join(self.recent)
        raise RuntimeError(
            f"{self.name}: timeout waiting for bestmove\n"
            f"{self.name} recent output:\n{recent}"
        )

    def quit(self):
        if self.proc.poll() is None:
            self._send("quit")
            try:
                self.proc.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                self.proc.kill()


class TimeControl:
    def __init__(self, wtime_ms, btime_ms, winc_ms=0, binc_ms=0, movestogo=None):
        self.wtime_ms = int(wtime_ms)
        self.btime_ms = int(btime_ms)
        self.winc_ms = int(winc_ms)
        self.binc_ms = int(binc_ms)
        self.movestogo = movestogo

    @classmethod
    def from_env(cls, movetime_ms):
        wtime_env = os.environ.get("WTIME_MS")
        btime_env = os.environ.get("BTIME_MS")
        if wtime_env is None and btime_env is None:
            return None
        wtime_ms = int(wtime_env or btime_env or movetime_ms * 40)
        btime_ms = int(btime_env or wtime_ms)
        winc_ms = int(os.environ.get("WINC_MS", "0"))
        binc_ms = int(os.environ.get("BINC_MS", "0"))
        movestogo = os.environ.get("MOVESTOGO")
        movestogo_val = int(movestogo) if movestogo is not None else None
        return cls(wtime_ms, btime_ms, winc_ms, binc_ms, movestogo_val)

    def go_command(self):
        parts = [
            "go",
            f"wtime {self.wtime_ms}",
            f"btime {self.btime_ms}",
            f"winc {self.winc_ms}",
            f"binc {self.binc_ms}",
        ]
        if self.movestogo is not None:
            parts.append(f"movestogo {self.movestogo}")
        return " ".join(parts)

    def apply_move(self, is_white, elapsed_ms):
        if is_white:
            self.wtime_ms = max(0, self.wtime_ms - elapsed_ms + self.winc_ms)
        else:
            self.btime_ms = max(0, self.btime_ms - elapsed_ms + self.binc_ms)

    def summary(self):
        return (
            f"wtime_ms: {self.wtime_ms}, btime_ms: {self.btime_ms}, "
            f"winc_ms: {self.winc_ms}, binc_ms: {self.binc_ms}, movestogo: {self.movestogo}"
        )


def main():
    stockfish = os.environ.get("STOCKFISH", "stockfish")
    engine = os.environ.get("ENGINE", "./target/debug/chess_engine")
    plies = int(os.environ.get("PLIES", "6"))
    movetime_ms = int(os.environ.get("MOVETIME_MS", "100"))
    bestmove_timeout = float(os.environ.get("BESTMOVE_TIMEOUT", "10"))
    engine_opts = os.environ.get("ENGINE_OPTS", "")
    stockfish_opts = os.environ.get("STOCKFISH_OPTS", "")

    print(f"engine: {engine}")
    print(f"stockfish: {stockfish}")
    time_control = TimeControl.from_env(movetime_ms)
    if time_control is not None:
        print(f"plies: {plies}, {time_control.summary()}")
    else:
        print(f"plies: {plies}, movetime_ms: {movetime_ms}")

    chess = UCIEngine(engine, "engine")
    sf = UCIEngine(stockfish, "stockfish")

    if engine_opts:
        for opt in engine_opts.split(";"):
            if "=" not in opt:
                continue
            name, value = opt.split("=", 1)
            chess.setoption(name.strip(), value.strip())
        chess._send("isready")
        chess._drain_until("readyok", timeout=5.0)

    if stockfish_opts:
        for opt in stockfish_opts.split(";"):
            if "=" not in opt:
                continue
            name, value = opt.split("=", 1)
            sf.setoption(name.strip(), value.strip())
        sf._send("isready")
        sf._drain_until("readyok", timeout=5.0)

    moves = []
    use_time_control = time_control is not None
    try:
        for ply in range(plies):
            side = chess if ply % 2 == 0 else sf
            if use_time_control:
                start = time.time()
                move = side.bestmove(moves, time_control=time_control, timeout=bestmove_timeout)
                elapsed_ms = int((time.time() - start) * 1000)
                time_control.apply_move(ply % 2 == 0, elapsed_ms)
                print(
                    "ply {}: {} -> {} (elapsed_ms={}, wtime_ms={}, btime_ms={})".format(
                        ply + 1,
                        side.name,
                        move,
                        elapsed_ms,
                        time_control.wtime_ms,
                        time_control.btime_ms,
                    )
                )
            else:
                start = time.time()
                move = side.bestmove(moves, movetime_ms=movetime_ms, timeout=bestmove_timeout)
                elapsed_ms = int((time.time() - start) * 1000)
                print(
                    "ply {}: {} -> {} (elapsed_ms={})".format(
                        ply + 1,
                        side.name,
                        move,
                        elapsed_ms,
                    )
                )
            if move in ("0000", "(none)"):
                print("engine returned null move; stopping")
                break
            moves.append(move)
    finally:
        chess.quit()
        sf.quit()

    print("moves:", " ".join(moves))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        sys.exit(1)
