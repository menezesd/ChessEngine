#!/usr/bin/env python3
"""Fit 21 coarse HCE weights jointly with 768 tapered per-square PST weights.

The feature representation matches the engine exactly:
- 21 coarse features in centipawns (CSV from dump_hce_features.py), scaled by
  ``--scale``, with tempo fixed as in fit_hce_weights.py (y = sf_cp - tempo).
- 384 signed occupancy features per phase (6 piece types x 64 squares) from
  White's perspective: White pieces +1 at their square, Black pieces -1 at the
  file-mirrored square. Tapered to match the engine's PhaseFactors:
  mg block x occ * ph, eg block x occ * (1 - ph) * (1 + eg2), where
  ph = midphase/24, eg2 = 1 when either side has only pawns (endgame_mult = 2).

Priors: CURRENT_WEIGHTS for the coarse block, current pst.rs tables
(MATERIAL + PST) / scale for the PST block.

Fold-in contract (exact, no engine logic change needed):
  base weight -> 1000 (1.0)
  MATERIAL_MG'[p]        = w_base * MATERIAL_MG[p]
  PST_MG'[p][sq]         = w_base * PST_MG[p][sq]  + w_pst_mg[p*64+sq]  * scale
  PST_EG'[p][sq]         = w_base * PST_EG[p][sq]  + w_pst_eg[p*64+sq]  * scale
then eval_mg/eval_eg recomputed incrementally reproduce the fitted model,
because taper(eval) = eval_mg*ph + endgame_mult*eval_eg*(1-ph) is linear in
the tables. Rounding: tables to i32 cp, coarse weights to permille.
"""

import argparse
import re
from pathlib import Path

import pandas as pd
import torch
import torch.nn.functional as F

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
    """Return [(piece_idx, signed_index)] from White's perspective.

    White pieces: +1 at their square (0..383). Black pieces: -1 at the
    file-mirrored square (PeSTO convention used by the engine's tables).
    """
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
            if ch.isupper():
                sq = rank * 8 + file
                out.append((p_idx, p_idx * 64 + sq))
            else:
                sq = rank * 8 + (7 - file)
                out.append((p_idx, -(p_idx * 64 + sq)))
            file += 1
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description="Fit 21 coarse + 768 tapered PST weights.")
    parser.add_argument("--features", default="runs/hce_tune/d14_pilot/mix_sf14_features.csv")
    parser.add_argument("--labels", default="runs/hce_tune/d14_pilot/mix_sf14.txt")
    parser.add_argument("--pst", default="src/board/pst.rs", help="Path to pst.rs (prior tables)")
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

    mat_mg, mat_eg, pst_mg, pst_eg = parse_pst_tables(args.pst)
    for p in range(6):
        assert len(pst_mg[p]) == 64 and len(pst_eg[p]) == 64
    coarse_prior = torch.tensor(CURRENT_WEIGHTS_PERMILLE, dtype=torch.float32) / 1000.0
    # PST prior is the per-square PST table only. Material lives exclusively in
    # the `base` coarse feature (eval_mg/eval_eg already sums MATERIAL + PST),
    # so including MATERIAL here would double-count it in the engine fold.
    pst_prior = torch.tensor(
        [pst_mg[p][sq] for p in range(6) for sq in range(64)]
        + [pst_eg[p][sq] for p in range(6) for sq in range(64)],
        dtype=torch.float32,
    ) / args.scale
    n_pst = 6 * 64

    frame = pd.read_csv(args.features)
    feature_cols = [c for c in frame.columns if c not in {"result", "sf_cp", "tempo", "phase", "full_white"}]
    assert len(feature_cols) == 21, f"expected 21 coarse columns, got {len(feature_cols)}"
    print(f"loaded {len(frame):,} feature rows, 21 coarse columns in order {feature_cols}")

    fens: list[str] = []
    labels: dict[str, float] = {}
    with open(args.labels) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            fen, sf_cp, _result = line.split(" | ")
            fens.append(fen)
            labels[fen] = float(sf_cp)
    assert len(fens) == len(frame), "labels and features must be row-aligned"

    rows_x: list[torch.Tensor] = []
    rows_y: list[float] = []
    tempo_v = frame["tempo"].to_numpy()
    full_v = frame["full_white"].to_numpy()
    coarse_v = frame[feature_cols].to_numpy(dtype="float32") / args.scale

    excluded = 0
    for i, fen in enumerate(fens):
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
        for p_idx, signed_idx in fen_signed_squares(fen):
            idx = abs(signed_idx)
            sign = 1 if signed_idx >= 0 else -1
            x_pst[idx] += sign * ph
            x_pst[n_pst + idx] += sign * (1 - ph) * (1 + eg2)
        rows_x.append(torch.cat([torch.from_numpy(coarse_v[i]), x_pst]))
        sf_cp = min(max(labels[fen], -args.clip_cp), args.clip_cp)
        rows_y.append((sf_cp - tempo_v[i]) / args.scale)
    if excluded:
        print(f"excluded {excluded:,} draw-scaled rows")

    x = torch.stack(rows_x)
    y = torch.tensor(rows_y, dtype=torch.float32)
    n, d = x.shape
    print(f"built {n:,} x {d} design matrix")

    generator = torch.Generator().manual_seed(args.seed)
    permutation = torch.randperm(n, generator=generator)
    validation_rows = int(n * args.validation_fraction)
    validation_idx = permutation[:validation_rows]
    train_idx = permutation[validation_rows:]
    train_x, train_y = x[train_idx], y[train_idx]
    validation_x, validation_y = x[validation_idx], y[validation_idx]

    prior = torch.cat([coarse_prior, pst_prior])
    prior_strength = args.l2 * len(train_x) / d
    reg = prior_strength * torch.eye(d, dtype=train_x.dtype)
    if args.fix_base or args.fix_coarse:
        # The `base` coarse feature (col 0) is a linear combination of the 768
        # PST occupancy features (base == sum over pieces of MATERIAL+PST), so
        # fitting base and the PST block jointly is unidentifiable. fix_base
        # holds only base at its prior; fix_coarse holds all 21 coarse weights
        # at their current tuned values so the PST block is a pure per-square
        # refinement that cannot compensate for coarse-feature moves.
        fixed_mask = torch.zeros(d, dtype=torch.bool)
        fixed_mask[0] = True
        if args.fix_coarse:
            fixed_mask[:21] = True
        residual = train_y - (train_x[:, fixed_mask] @ prior[fixed_mask])
        free_idx = ~fixed_mask
        lhs = train_x[:, free_idx].T @ train_x[:, free_idx] + reg[free_idx][:, free_idx]
        rhs = train_x[:, free_idx].T @ residual + prior_strength * prior[free_idx]
        solution = torch.zeros(d)
        solution[fixed_mask] = prior[fixed_mask]
        solution[free_idx] = torch.linalg.solve(lhs, rhs)
    else:
        lhs = train_x.T @ train_x + reg
        rhs = train_x.T @ train_y + prior_strength * prior
        solution = torch.linalg.solve(lhs, rhs)

    if args.zero_sum:
        # Enforce sum(w_pst[p*64:(p+1)*64]) == 0 per piece per phase via
        # equality-constrained ridge: min ||Xw - y||^2 + reg on w subject to
        # A w = 0. Solved with KKT (augmented system) over the free columns.
        fixed_mask = torch.zeros(d, dtype=torch.bool)
        if args.fix_base:
            fixed_mask[0] = True
        if args.fix_coarse:
            fixed_mask[:21] = True
        free_idx = torch.nonzero(~fixed_mask).squeeze(1)
        free_x = train_x[:, free_idx]
        k = len(free_idx)
        c = torch.zeros(12, k)
        for p in range(6):
            mg_cols = [i for i, j in enumerate(free_idx.tolist()) if 21 + p * 64 <= j < 21 + (p + 1) * 64]
            eg_cols = [i for i, j in enumerate(free_idx.tolist()) if 21 + n_pst + p * 64 <= j < 21 + n_pst + (p + 1) * 64]
            for i in mg_cols:
                c[p, i] = 1.0
            for i in eg_cols:
                c[6 + p, i] = 1.0
        reg_free = reg[free_idx][:, free_idx]
        kkt = torch.zeros(k + 12, k + 12)
        kkt[:k, :k] = free_x.T @ free_x + reg_free
        kkt[:k, k:] = c.T
        kkt[k:, :k] = c
        kkt_rhs = torch.zeros(k + 12)
        residual = train_y - (train_x[:, fixed_mask] @ prior[fixed_mask])
        kkt_rhs[:k] = free_x.T @ residual + prior_strength * prior[free_idx]
        free_solution = torch.linalg.solve(kkt, kkt_rhs)[:k]
        full = prior.clone()
        full[free_idx] = free_solution
        solution = full

    for label, w in [("joint", solution), ("coarse-only", torch.cat([solution[:21], torch.zeros(2 * n_pst)]))]:
        pred = validation_x @ w * args.scale
        target = validation_y * args.scale
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
