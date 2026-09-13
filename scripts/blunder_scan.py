#!/usr/bin/env python3
import os
import sys

if __package__:
    from .uci_client import UCIClient, exact_score, SCORE_MATE_RE
else:
    from uci_client import UCIClient, exact_score, SCORE_MATE_RE


class UCIEngine(UCIClient):
    def __init__(self, path, name):
        self.path = path
        self.name = name
        super().__init__(path, [])

    _send = UCIClient.send

    def _drain_until(self, token, timeout):
        self.wait_for(token, timeout)
        return True

    def setoption(self, name, value):
        self._send(f"setoption name {name} value {value}")

    def position(self, moves):
        if moves:
            self._send("position startpos moves " + " ".join(moves))
        else:
            self._send("position startpos")

    def go(self, movetime_ms):
        return self._search(
            f"go movetime {movetime_ms}", max(5.0, movetime_ms / 1000 + 2)
        )

    def go_searchmoves(self, movetime_ms, move):
        return self._search(
            f"go movetime {movetime_ms} searchmoves {move}",
            max(5.0, movetime_ms / 1000 + 2),
        )

    def _search(self, command, timeout):
        last_score = None
        for line in self.search_lines(command, timeout):
            score = exact_score(line, primary_only=True)
            if score is not None:
                if SCORE_MATE_RE.search(line.partition(" string ")[0]):
                    score = 100000 if score > 0 else -100000
                last_score = score
            if line.startswith("bestmove "):
                return last_score, line.split()[1]


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
    try:
        sf.setoption("MultiPV", "2")
        sf._send("isready")
        sf._drain_until("readyok", timeout=5.0)
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
            # Both searches evaluate the position before the played move.
            score_after = move_score
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
