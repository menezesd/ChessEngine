#!/usr/bin/env python3
"""
Fit piece-square tables from Stockfish-labelled positions (FEN | sf_cp | result).

Feature representation: 384 occupancy features (6 piece types x 64 squares) from
White's perspective, with black pieces mirrored across the file axis (PeSTO
convention). Ridge regression on clipped sf_cp, matching the HCE fitter's
scale/clip/validation conventions for comparability.

The result is a self-contained linear PST model (material is absorbed into the
tables). Compare its held-out RMSE against the 21-feature HCE fit (227.5 cp on
the same d14 pilot data) to judge whether per-square weights add signal over the
single coarse `base` multiplier.
"""

import argparse
from pathlib import Path

import torch
import torch.nn.functional as F

PIECES = "PNBRQK"  # index 0..5, matches Piece::index()
FILE_LETTERS = "abcdefgh"
PIECE_NAMES = ["pawn", "knight", "bishop", "rook", "queen", "king"]


def fen_to_squares(fen: str) -> list[int]:
    """Return white-perspective square indices (0..383) for every piece."""
    board_part = fen.split()[0]
    squares: list[int] = []
    rank = 7
    file = 0
    for ch in board_part:
        if ch == "/":
            rank -= 1
            file = 0
        elif ch.isdigit():
            file += int(ch)
        else:
            p_idx = PIECES.index(ch.upper())
            # White perspective: black pieces mirror across the file axis.
            if ch.isupper():
                sq = rank * 8 + file
            else:
                sq = rank * 8 + (7 - file)
            squares.append(p_idx * 64 + sq)
            file += 1
    return squares


def square_name(sq: int) -> str:
    return FILE_LETTERS[sq % 8] + str(sq // 8 + 1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Fit PST occupancy weights from labelled FENs.")
    parser.add_argument("--input", default="runs/hce_tune/d14_pilot/mix_sf14.txt",
                        help="Lines of 'FEN | sf_cp | result'")
    parser.add_argument("--output", default="runs/hce_tune/d14_pilot/pst_ridge.csv")
    parser.add_argument("--scale", type=float, default=400.0,
                        help="Feature/target scaling matching fit_hce_weights.py")
    parser.add_argument("--clip-cp", type=float, default=2000.0)
    parser.add_argument("--l2", type=float, default=0.02,
                        help="Prior strength as a fraction (mean-squared prior on zero)")
    parser.add_argument("--validation-fraction", type=float, default=0.15)
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    rows_x: list[torch.Tensor] = []
    rows_y: list[float] = []
    with open(args.input) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            fen, sf_cp, _result = line.split(" | ")
            vec = torch.zeros(384)
            for sq in fen_to_squares(fen):
                vec[sq] += 1.0
            rows_x.append(vec)
            rows_y.append(float(sf_cp))

    x = torch.stack(rows_x) / args.scale
    y = torch.clamp(torch.tensor(rows_y), -args.clip_cp, args.clip_cp) / args.scale
    n, d = x.shape
    print(f"loaded {n:,} positions, {d} PST features")

    generator = torch.Generator().manual_seed(args.seed)
    permutation = torch.randperm(n, generator=generator)
    validation_rows = int(n * args.validation_fraction)
    validation_idx = permutation[:validation_rows]
    train_idx = permutation[validation_rows:]
    train_x, train_y = x[train_idx], y[train_idx]
    validation_x, validation_y = x[validation_idx], y[validation_idx]

    # Match the HCE fitter's normal-equation form: l2 * rows / weights.
    prior_strength = args.l2 * len(train_x) / d
    reg = prior_strength * torch.eye(d, dtype=train_x.dtype)
    lhs = train_x.T @ train_x + reg
    rhs = train_x.T @ train_y
    solution = torch.linalg.solve(lhs, rhs)

    for label, w in [("fit", solution), ("zeros", torch.zeros(d))]:
        pred = validation_x @ w * args.scale
        target = validation_y * args.scale
        rmse = torch.sqrt(F.mse_loss(pred, target))
        print(f"held-out RMSE ({label}): {rmse.item():.1f} cp")

    # Report the learned tables.
    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w") as f:
        f.write("piece,square,weight\n")
        for p in range(6):
            base = p * 64
            for sq in range(64):
                f.write(f"{PIECE_NAMES[p]},{square_name(sq)},{solution[base + sq].item() * args.scale:.1f}\n")
    print(f"wrote {out}")

    print("\nper-piece value at a1 corner vs d4 centre:")
    for p in range(6):
        corner = solution[p * 64 + 0].item() * args.scale
        centre = solution[p * 64 + 27].item() * args.scale
        print(f"  {PIECE_NAMES[p]:>8}: corner {corner:8.1f}  d4 {centre:8.1f}")


if __name__ == "__main__":
    main()
