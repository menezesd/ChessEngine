"""UCI transport shared by the training-data collectors."""

import queue
import re
import subprocess
import threading
import time


SCORE_CP_RE = re.compile(r"\bscore cp (-?\d+)")
SCORE_MATE_RE = re.compile(r"\bscore mate (-?\d+)")
MULTIPV_RE = re.compile(r"\bmultipv (\d+)")


def exact_score(line: str, *, primary_only: bool = False) -> int | None:
    # Everything after the UCI string field is diagnostic text.
    line = line.partition(" string ")[0]
    if (
        not line.startswith("info ")
        or {"lowerbound", "upperbound"}.intersection(line.split())
    ):
        return None
    variation = MULTIPV_RE.search(line)
    if primary_only and variation and int(variation.group(1)) != 1:
        return None
    if match := SCORE_CP_RE.search(line):
        return int(match.group(1))
    if match := SCORE_MATE_RE.search(line):
        mate = int(match.group(1))
        magnitude = max(1, 30000 - abs(mate) * 100)
        # UCI mate 0 means the side to move is already checkmated.
        return magnitude if mate > 0 else -magnitude
    return None


class UCIClient:
    def __init__(self, path: str, options: list[str]):
        self.proc = subprocess.Popen(
            [path], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, text=True, bufsize=1,
        )
        self.lines: queue.Queue[str | None] = queue.Queue()
        self.reader_thread = threading.Thread(target=self._reader, daemon=True)
        self.reader_thread.start()
        try:
            self.send("uci")
            self.wait_for("uciok")
            for option in options:
                self.send(f"setoption name {option}")
            self.send("isready")
            self.wait_for("readyok")
        except BaseException:
            self.quit()
            raise

    def _reader(self):
        try:
            for line in self.proc.stdout:
                self.lines.put(line.rstrip("\r\n"))
        except (OSError, ValueError):
            pass
        finally:
            self.lines.put(None)

    def send(self, command: str):
        self.proc.stdin.write(command + "\n")
        self.proc.stdin.flush()

    def read_line(self, deadline: float) -> str:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("UCI response deadline expired")
        try:
            line = self.lines.get(timeout=remaining)
        except queue.Empty as error:
            raise TimeoutError("UCI response deadline expired") from error
        if line is None:
            self.lines.put(None)
            raise RuntimeError("engine closed its output")
        return line

    def wait_for(self, token: str, timeout: float = 10.0):
        deadline = time.monotonic() + timeout
        while True:
            line = self.read_line(deadline)
            if line.split(maxsplit=1)[:1] == [token]:
                return line

    def search_lines(self, command: str, timeout: float):
        """Consume exactly one search, stopping and draining it on timeout."""
        self.send(command)
        deadline = time.monotonic() + timeout
        stopping = False
        while True:
            try:
                line = self.read_line(deadline)
            except TimeoutError:
                if stopping:
                    self.quit()
                    raise TimeoutError("engine did not finish after stop") from None
                self.send("stop")
                stopping = True
                deadline = time.monotonic() + 2.0
                continue
            yield line
            if line.split(maxsplit=1)[:1] == ["bestmove"]:
                return

    def score_position(self, fen: str, command: str, timeout: float = 60.0, *, new_game=True):
        """Return the last exact primary score and whether it represents mate."""
        if new_game:
            self.send("ucinewgame")
        self.send(f"position fen {fen}")
        score, is_mate = None, False
        for line in self.search_lines(command, timeout):
            if (value := exact_score(line, primary_only=True)) is not None:
                score = value
                is_mate = SCORE_MATE_RE.search(line.partition(" string ")[0]) is not None
        return score, is_mate

    def quit(self):
        try:
            if self.proc.poll() is None:
                try:
                    self.send("quit")
                except (BrokenPipeError, OSError):
                    pass
                try:
                    self.proc.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    self.proc.kill()
                    self.proc.wait()
        finally:
            self.reader_thread.join(timeout=1)
            self.proc.stdin.close()
            self.proc.stdout.close()
