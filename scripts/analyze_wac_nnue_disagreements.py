#!/usr/bin/env python3
"""Compare HCE and NNUE WAC decisions and print static eval context."""

import argparse
from dataclasses import dataclass

from wac_eval_nnue import UCIEngine, parse_epd, uci_to_san


@dataclass
class Decision:
    move: str
    san: str
    correct: bool
    eval_line: str


class EvalEngine(UCIEngine):
    def static_eval(self, fen: str) -> str:
        self._send(f"position fen {fen}")
        self._send("eval")
        while True:
            line = self.lines.get(timeout=5.0)
            if line.startswith("info string eval "):
                return line


def decide(engine: EvalEngine, fen: str, expected: list[str], movetime_ms: int, depth: int) -> Decision:
    move = engine.bestmove(fen, movetime_ms, depth)
    san = uci_to_san(fen, move)
    return Decision(
        move=move,
        san=san,
        correct=san in expected or move in expected,
        eval_line=engine.static_eval(fen),
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--epd", default="tests/data/wac_all.epd")
    parser.add_argument("--limit", type=int, default=201)
    parser.add_argument("--movetime-ms", type=int, default=500)
    parser.add_argument("--depth", type=int, default=0)
    parser.add_argument("--max-print", type=int, default=50)
    args = parser.parse_args()

    hce = EvalEngine(
        args.engine,
        options=["UseNNUE value false"],
    )
    nnue = EvalEngine(
        args.engine,
        options=["UseNNUE value true", "NnueHceBlend value 100"],
    )

    total = hce_correct = nnue_correct = printed = 0
    try:
        with open(args.epd) as f:
            for line in f:
                if not line.strip() or total >= args.limit:
                    break
                fen, expected, epd_id = parse_epd(line)
                if fen is None:
                    continue

                total += 1
                h = decide(hce, fen, expected, args.movetime_ms, args.depth)
                n = decide(nnue, fen, expected, args.movetime_ms, args.depth)
                hce_correct += int(h.correct)
                nnue_correct += int(n.correct)

                if h.correct != n.correct and printed < args.max_print:
                    printed += 1
                    print(f"{epd_id}: expected={','.join(expected)}")
                    print(f"  HCE : {h.move}/{h.san} correct={h.correct} {h.eval_line}")
                    print(f"  NNUE: {n.move}/{n.san} correct={n.correct} {n.eval_line}")

        print(f"summary hce={hce_correct}/{total} nnue={nnue_correct}/{total}")
    finally:
        hce.quit()
        nnue.quit()


if __name__ == "__main__":
    main()
