#!/usr/bin/env python3
import collections
import queue
import subprocess
import sys
import threading
import time


class UCIEngine:
    def __init__(self, path):
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
        raise RuntimeError(f"timeout waiting for {token}")

    def command(self, cmd):
        self._send(cmd)

    def read_line(self, timeout=5.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                return self.lines.get(timeout=0.1)
            except queue.Empty:
                if self.proc.poll() is not None:
                    recent = "\n".join(self.recent)
                    raise RuntimeError(
                        f"engine exited with {self.proc.returncode}\nrecent:\n{recent}"
                    )
                continue
        raise RuntimeError("timeout waiting for engine output")

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
        print("usage: check_mate_status.py <move1> <move2> ...", file=sys.stderr)
        print("or: echo \"e2e4 e7e5 ...\" | check_mate_status.py", file=sys.stderr)
        return 2

    engine = UCIEngine("./target/debug/chess_engine")
    try:
        engine.command("position startpos moves " + " ".join(moves))
        engine.command("d")
        info_lines = []
        while True:
            line = engine.read_line(timeout=2.0)
            info_lines.append(line)
            if line.startswith("Legal moves:"):
                break
        for line in info_lines:
            print(line)
    finally:
        engine.quit()
    return 0


if __name__ == "__main__":
    sys.exit(main())
