#!/usr/bin/env python3
"""Dump coarse HCE features for sampled FEN/result or FEN/score/result rows."""

from __future__ import annotations

import argparse
import queue
import re
import subprocess
import threading
from pathlib import Path


FEATURE_RE = re.compile(
    r"evalfeatures names (?P<names>\S+) values (?P<values>[-0-9,]+) "
    r"tempo (?P<tempo>-?\d+) phase (?P<phase>-?\d+) full_white (?P<full>-?\d+)"
)


class Engine:
    def __init__(self, path: str):
        self.proc = subprocess.Popen(
            [path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        self.lines: queue.Queue[str] = queue.Queue()
        threading.Thread(target=self._reader, daemon=True).start()
        self.send("uci")
        self.wait_for("uciok")
        self.send("isready")
        self.wait_for("readyok")

    def _reader(self) -> None:
        assert self.proc.stdout is not None
        for line in self.proc.stdout:
            self.lines.put(line.rstrip("\n"))

    def send(self, cmd: str) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def wait_for(self, token: str, timeout: float = 10.0) -> str:
        import time

        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if token in line:
                return line
        raise RuntimeError(f"timeout waiting for {token}")

    def features(self, fen: str) -> tuple[list[str], list[int], int, int, int]:
        self.send(f"position fen {fen}")
        self.send("evalfeatures")
        line = self.wait_for("evalfeatures")
        match = FEATURE_RE.search(line)
        if not match:
            raise RuntimeError(f"could not parse evalfeatures line: {line}")
        names = match.group("names").split(",")
        values = [int(v) for v in match.group("values").split(",")]
        tempo = int(match.group("tempo"))
        phase = int(match.group("phase"))
        full = int(match.group("full"))
        return names, values, tempo, phase, full

    def quit(self) -> None:
        if self.proc.poll() is None:
            self.send("quit")
            self.proc.wait(timeout=2)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--limit", type=int, default=0)
    args = parser.parse_args()

    engine = Engine(args.engine)
    written = 0
    try:
        with args.input.open() as inp, args.output.open("w") as out:
            header_written = False
            for line in inp:
                if args.limit and written >= args.limit:
                    break
                line = line.strip()
                if not line or "|" not in line:
                    continue
                parts = [part.strip() for part in line.split("|")]
                if len(parts) == 2:
                    fen, result = parts
                    sf_cp = None
                elif len(parts) == 3:
                    fen, sf_cp, result = parts
                else:
                    continue
                names, values, tempo, phase, full = engine.features(fen)
                if not header_written:
                    columns = ["result"]
                    if sf_cp is not None:
                        columns.append("sf_cp")
                    columns.extend(["tempo", "phase", "full_white", *names])
                    out.write(",".join(columns) + "\n")
                    header_written = True
                row = [f"{float(result):.1f}"]
                if sf_cp is not None:
                    row.append(str(int(float(sf_cp))))
                row.extend([str(tempo), str(phase), str(full), *[str(v) for v in values]])
                out.write(",".join(row) + "\n")
                written += 1
                if written % 1000 == 0:
                    print(f"dumped {written:,}", flush=True)
    finally:
        engine.quit()
    print(f"dumped {written:,} feature rows -> {args.output}")


if __name__ == "__main__":
    main()
