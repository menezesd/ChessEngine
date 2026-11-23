#!/usr/bin/env python3
"""Fit 21 coarse HCE weights jointly with 768 tapered per-square PST weights.

The feature representation matches the engine exactly:
- 21 coarse features in centipawns (CSV from dump_hce_features.py), scaled by
  ``--scale``, with tempo fixed as in fit_hce_weights.py (y = sf_cp - tempo).
- 384 signed occupancy features per phase (6 piece types x 64 squares) from
  White's perspective: White pieces +1 at their square, Black pieces -1 at the
  rank-mirrored square. Tapered to match the engine's PhaseFactors:
  mg block x occ * ph, eg block x occ * (1 - ph) * (1 + eg2), where
  ph = midphase/24, eg2 = 1 when either side has only pawns (endgame_mult = 2).

The coarse prior is CURRENT_WEIGHTS. The PST prior is zero because the base
feature already includes the existing material and piece-square scores.

Fold-in contract (exact, no engine logic change needed):
  base weight -> 1000 (1.0)
  MATERIAL_MG'[p]        = w_base * MATERIAL_MG[p]
  MATERIAL_EG'[p]        = w_base * MATERIAL_EG[p]
  PST_MG'[p][sq]         = w_base * PST_MG[p][sq]  + w_pst_mg[p*64+sq]  * scale
  PST_EG'[p][sq]         = w_base * PST_EG[p][sq]  + w_pst_eg[p*64+sq]  * scale
then eval_mg/eval_eg recomputed incrementally reproduce the fitted model,
because taper(eval) = eval_mg*ph + endgame_mult*eval_eg*(1-ph) is linear in
the tables. Rounding: tables to i32 cp, coarse weights to permille.
"""

import argparse
import csv
import random
import re
from pathlib import Path

PIECES = "PNBRQK"
FILE_LETTERS = "abcdefgh"
PIECE_NAMES = ["pawn", "knight", "bishop", "rook", "queen", "king"]

CURRENT_WEIGHTS_PERMILLE = [
    1078, 1052, 1091, 978, 1052, 1046, 1069, 1001, 1002, 1094, 1134, 972, 1019, 967, 1035, 1009,
    1029, 975, 1004, 1005, 1005,
]


def square_name(sq: int) -> str:
    return FILE_LETTERS[sq % 8] + str(sq // 8 + 1)


def parse_pst_tables(pst_path: str) -> tuple[list[int], list[int], list[list[int]], list[list[int]]]:
    text = re.sub(r"//.*", "", Path(pst_path).read_text())

    def parse_int_array(block: str) -> list[int]:
        return [int(x) for x in re.findall(r"-?\d+", block)]

    def parse_piece_array(block: str) -> list[list[int]]:
        pieces = []
        for m in re.finditer(r"\[([-0-9,\s]+)\]", block):
            pieces.append(parse_int_array(m.group(1)))
        return pieces

    mat_mg = parse_int_array(re.search(r"MATERIAL_MG: \[i32; 6\] = (\[[^\]]*\])", text).group(1))
    mat_eg = parse_int_array(re.search(r"MATERIAL_EG: \[i32; 6\] = (\[[^\]]*\])", text).group(1))
    pst_mg = parse_piece_array(re.search(r"PST_MG: \[\[i32; 64\]; 6\] = (\[.*?\n\])", text, re.S).group(1))
    pst_eg = parse_piece_array(re.search(r"PST_EG: \[\[i32; 64\]; 6\] = (\[.*?\n\])", text, re.S).group(1))
    return mat_mg, mat_eg, pst_mg, pst_eg


def fen_phase(fen: str) -> tuple[int, int]:
    phase_weights = [0, 1, 1, 2, 4, 0]
    board_part = fen.split()[0]
    white_phase = black_phase = 0
    for ch in board_part:
        if ch.isalpha():
            p = PIECES.index(ch.upper())
            if ch.isupper():
                white_phase += phase_weights[p]
            else:
                black_phase += phase_weights[p]
    return white_phase, black_phase


def fen_signed_squares(fen: str) -> list[tuple[int, int]]:
    """Return (PST index, sign) pairs from White's perspective."""
    board_part = fen.split()[0]
    out: list[tuple[int, int]] = []
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
            square = rank * 8 + file
            if ch.isupper():
                out.append((p_idx * 64 + square, 1))
            else:
                out.append((p_idx * 64 + (square ^ 56), -1))
            file += 1
    return out


def split_positions(fens: list[str], fraction: float, seed: int) -> tuple[list[int], list[int]]:
    """Keep repeated positions together even when their move counters differ."""
    if not 0 <= fraction < 1:
        raise ValueError("validation fraction must be at least zero and less than one")
    keys = [" ".join(fen.split()[:4]) for fen in fens]
    groups = list(dict.fromkeys(keys))
    if not groups or (fraction > 0 and len(groups) < 2):
        raise ValueError("training and validation require distinct usable positions")
    random.Random(seed).shuffle(groups)
    count = min(len(groups) - 1, max(1, int(len(groups) * fraction))) if fraction else 0
    holdout = set(groups[:count])
    train, validation = [], []
    for index, key in enumerate(keys):
        (validation if key in holdout else train).append(index)
    return train, validation


def main() -> None:
    parser = argparse.ArgumentParser(description="Fit 21 coarse + 768 tapered PST weights.")
    parser.add_argument("--features", default="runs/hce_tune/d14_pilot/mix_sf14_features.csv")
    parser.add_argument("--labels", default="runs/hce_tune/d14_pilot/mix_sf14.txt")
    parser.add_argument("--pst", default="src/board/pst.rs", help="Path to pst.rs for table layout validation")
    parser.add_argument("--output-prefix", default="runs/hce_tune/d14_pilot/joint")
    parser.add_argument("--scale", type=float, default=400.0)
    parser.add_argument("--clip-cp", type=float, default=2000.0)
    parser.add_argument("--l2", type=float, default=1e-4)
    parser.add_argument("--fix-base", action="store_true",
                        help="Hold the base coarse weight at its current 1.078 and fit only "
                             "PST deltas + the other 20 weights (avoids base/PST collinearity)")
    parser.add_argument("--fix-coarse", action="store_true",
                        help="Hold ALL 21 coarse weights at their current values and fit only "
                             "the 768 PST deltas (PST as a pure per-square refinement)")
    parser.add_argument("--zero-sum", action="store_true",
                        help="Constrain each piece's 64 PST weights (mg and eg separately) to "
                             "sum to zero, so the PST block can only redistribute value across "
                             "squares and never change a piece's total material")
    parser.add_argument("--validation-fraction", type=float, default=0.15)
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    import numpy as np
    import torch
    import torch.nn.functional as F

    _, _, pst_mg, pst_eg = parse_pst_tables(args.pst)
    for p in range(6):
        assert len(pst_mg[p]) == 64 and len(pst_eg[p]) == 64
    coarse_prior = torch.tensor(CURRENT_WEIGHTS_PERMILLE, dtype=torch.float32) / 1000.0
    n_pst = 6 * 64
    pst_prior = torch.zeros(2 * n_pst)

    with open(args.features, newline="") as feature_file:
        header = next(csv.reader(feature_file))
        frame = np.loadtxt(feature_file, delimiter=",", dtype=np.float32, ndmin=2)
    columns = {name: index for index, name in enumerate(header)}
    feature_cols = [c for c in header if c not in {"result", "sf_cp", "tempo", "phase", "full_white"}]
    assert len(feature_cols) == 21, f"expected 21 coarse columns, got {len(feature_cols)}"
    print(f"loaded {len(frame):,} feature rows, 21 coarse columns in order {feature_cols}")

    positions: list[tuple[str, float]] = []
    with open(args.labels) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            fen, sf_cp, _result = (field.strip() for field in line.split("|"))
            positions.append((fen, float(sf_cp)))
    assert len(positions) == len(frame), "labels and features must be row-aligned"
    if "sf_cp" in columns:
        assert all(
            int(score) == feature_score
            for (_, score), feature_score in zip(positions, frame[:, columns["sf_cp"]])
        ), "label scores and feature rows differ"

    rows_x: list[torch.Tensor] = []
    rows_y: list[float] = []
    usable_fens: list[str] = []
    tempo_v = frame[:, columns["tempo"]]
    full_v = frame[:, columns["full_white"]]
    coarse_v = frame[:, [columns[name] for name in feature_cols]] / args.scale

    excluded = 0
    for i, (fen, sf_cp) in enumerate(positions):
        unscaled = coarse_v[i].sum() * args.scale + tempo_v[i]
        if abs(full_v[i] - unscaled) > 32.0:
            excluded += 1
            continue
        white_phase, black_phase = fen_phase(fen)
        midphase = min(white_phase + black_phase, 24)
        endphase = 24 - midphase
        eg2 = 1 if min(white_phase, black_phase) == 0 else 0
        ph = midphase / 24.0
        x_pst = torch.zeros(2 * n_pst)
        for idx, sign in fen_signed_squares(fen):
            x_pst[idx] += sign * ph
            x_pst[n_pst + idx] += sign * (1 - ph) * (1 + eg2)
        rows_x.append(torch.cat([torch.from_numpy(coarse_v[i]), x_pst]))
        sf_cp = min(max(sf_cp, -args.clip_cp), args.clip_cp)
        rows_y.append((sf_cp - tempo_v[i]) / args.scale)
        usable_fens.append(fen)
    if excluded:
        print(f"excluded {excluded:,} draw-scaled rows")

    try:
        train, validation = split_positions(usable_fens, args.validation_fraction, args.seed)
    except ValueError as error:
        parser.error(str(error))
    x = torch.stack(rows_x)
    y = torch.tensor(rows_y, dtype=torch.float32)
    n, d = x.shape
    print(f"built {n:,} x {d} design matrix")

    train_x, train_y = x[train], y[train]

    prior = torch.cat([coarse_prior, pst_prior])
    prior_strength = args.l2 * len(train_x) / d
    fixed = 21 if args.fix_coarse else int(args.fix_base)
    free_x = train_x[:, fixed:]
    residual = train_y - train_x[:, :fixed] @ prior[:fixed]
    lhs = free_x.T @ free_x
    lhs.diagonal().add_(prior_strength)
    rhs = free_x.T @ residual + prior_strength * prior[fixed:]

    if args.zero_sum:
        # Each phase has one zero-sum constraint per piece's 64 squares.
        free_count = d - fixed
        constraints = torch.zeros(12, free_count)
        pst_start = 21 - fixed
        for block in range(12):
            start = pst_start + block * 64
            constraints[block, start : start + 64] = 1
        kkt = torch.zeros(free_count + 12, free_count + 12)
        kkt[:free_count, :free_count] = lhs
        kkt[:free_count, free_count:] = constraints.T
        kkt[free_count:, :free_count] = constraints
        rhs = torch.cat([rhs, torch.zeros(12)])
        free_solution = torch.linalg.solve(kkt, rhs)[:free_count]
    else:
        free_solution = torch.linalg.solve(lhs, rhs)
    solution = torch.cat([prior[:fixed], free_solution])

    if validation:
        validation_x = x[validation]
        target = y[validation] * args.scale
        for label, w in [("joint", solution), ("coarse-only", torch.cat([solution[:21], pst_prior]))]:
            pred = validation_x @ w * args.scale
            rmse = torch.sqrt(F.mse_loss(pred, target))
            print(f"held-out RMSE ({label}): {rmse.item():.1f} cp")

    out = Path(args.output_prefix)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.with_name(out.name + "_coarse.csv").open("w") as f:
        f.write("name,weight_permille\n")
        for name, w in zip(feature_cols, solution[:21]):
            f.write(f"{name},{w.item() * 1000.0:.0f}\n")
    with out.with_name(out.name + "_pst.csv").open("w") as f:
        f.write("piece,square,mg,eg\n")
        for p in range(6):
            for sq in range(64):
                mg = solution[21 + p * 64 + sq].item() * args.scale
                eg = solution[21 + n_pst + p * 64 + sq].item() * args.scale
                f.write(f"{PIECE_NAMES[p]},{square_name(sq)},{mg:.1f},{eg:.1f}\n")
    print(f"wrote {out}_coarse.csv and {out}_pst.csv")

    print("\nfitted base weight: {:.4f}".format(solution[0].item()))
    print("largest coarse moves from prior:")
    deltas = (solution[:21] - coarse_prior).abs()
    for idx in deltas.argsort(descending=True)[:6]:
        print(f"  {feature_cols[idx]:>18} {coarse_prior[idx]:.3f} -> {solution[idx]:.3f}")


if __name__ == "__main__":
    main()
