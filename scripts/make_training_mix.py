#!/usr/bin/env python3
"""Build a shuffled NNUE training mix from large labeled text files."""

import argparse
import random


def reservoir_sample(path: str, n: int, seed: int) -> list[str]:
    rng = random.Random(seed)
    sample: list[str] = []
    with open(path) as f:
        for i, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            if len(sample) < n:
                sample.append(line)
            else:
                j = rng.randrange(i)
                if j < n:
                    sample[j] = line
    return sample


def read_repeated(path: str, repeat: int) -> list[str]:
    rows: list[str] = []
    with open(path) as f:
        base = [line.strip() for line in f if line.strip()]
    for _ in range(repeat):
        rows.extend(base)
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    parser.add_argument("--sample", action="append", nargs=2, metavar=("PATH", "N"))
    parser.add_argument("--repeat", action="append", nargs=2, metavar=("PATH", "COUNT"))
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    rows: list[str] = []
    for idx, (path, n) in enumerate(args.sample or []):
        rows.extend(reservoir_sample(path, int(n), args.seed + idx))
    for path, count in args.repeat or []:
        rows.extend(read_repeated(path, int(count)))

    random.Random(args.seed).shuffle(rows)
    with open(args.output, "w") as f:
        for row in rows:
            f.write(row + "\n")

    print(f"Wrote {len(rows):,} rows to {args.output}")


if __name__ == "__main__":
    main()
