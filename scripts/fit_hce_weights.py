#!/usr/bin/env python3
"""Fit representable HCE term weights from feature CSV labels.

The engine applies one multiplier per entry in ``HCE_FEATURE_NAMES`` and adds
tempo separately.  Keep the fitting model in that same form: tempo is removed
from a Stockfish centipawn target, and no unrepresentable intercept is emitted.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import pandas as pd
import torch
import torch.nn.functional as F


NON_FEATURE_COLUMNS = {"result", "sf_cp", "tempo", "phase", "full_white"}
HCE_FEATURE_NAMES = (
    "base",
    "bishop",
    "mobility",
    "pawn_structure",
    "king_safety",
    "king_shield",
    "rooks",
    "minor_pieces",
    "tropism",
    "passed_pawns",
    "hanging",
    "coordination",
    "pawn_advanced",
    "weak_squares",
    "king_danger",
    "endgame_patterns",
    "space_control",
    "threats_advanced",
    "piece_quality",
    "imbalances",
    "initiative",
)
CURRENT_WEIGHTS = torch.tensor(
    [
        1.078,
        1.052,
        1.091,
        0.978,
        1.052,
        1.046,
        1.069,
        1.001,
        1.002,
        1.094,
        1.134,
        0.972,
        1.019,
        0.967,
        1.035,
        1.009,
        1.029,
        0.975,
        1.004,
        1.005,
        1.005,
    ],
    dtype=torch.float32,
)


def write_weights(output: Path, names: list[str], weights: torch.Tensor) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("w") as out:
        out.write("name,weight\n")
        for name, weight in zip(names, weights.tolist()):
            out.write(f"{name},{weight:.6f}\n")


def print_largest_moves(names: list[str], weights: torch.Tensor) -> None:
    print("largest moves from the in-engine baseline:")
    moved = sorted(
        zip(names, weights.tolist(), CURRENT_WEIGHTS.tolist()),
        key=lambda item: abs(item[1] - item[2]),
        reverse=True,
    )
    for name, weight, baseline in moved[:12]:
        print(f"  {name:18s} {weight:.3f} ({weight - baseline:+.3f})")


def print_validation_metric(
    target: str,
    validation_x: torch.Tensor,
    validation_y: torch.Tensor,
    fixed_offset: torch.Tensor,
    weights: torch.Tensor,
    scale: float,
) -> None:
    if not len(validation_x):
        return

    with torch.no_grad():
        baseline_logits = validation_x @ CURRENT_WEIGHTS + fixed_offset
        candidate_logits = validation_x @ weights + fixed_offset
        if target == "result":
            baseline = F.binary_cross_entropy_with_logits(baseline_logits, validation_y)
            candidate = F.binary_cross_entropy_with_logits(candidate_logits, validation_y)
            print(
                f"held-out BCE: {candidate.item():.6f} "
                f"(baseline {baseline.item():.6f}; {len(validation_x):,} rows)"
            )
        else:
            baseline = torch.sqrt(F.mse_loss(baseline_logits, validation_y)) * scale
            candidate = torch.sqrt(F.mse_loss(candidate_logits, validation_y)) * scale
            print(
                f"held-out RMSE: {candidate.item():.1f} cp "
                f"(baseline {baseline.item():.1f} cp; {len(validation_x):,} rows)"
            )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--epochs", type=int, default=2000)
    parser.add_argument("--batch-size", type=int, default=0)
    parser.add_argument("--lr", type=float, default=0.03)
    parser.add_argument(
        "--l2",
        type=float,
        default=0.02,
        help="Mean-squared penalty toward the current engine weights.",
    )
    parser.add_argument("--scale", type=float, default=400.0)
    parser.add_argument("--target", choices=("result", "sf_cp"), default="result")
    parser.add_argument("--clip-cp", type=float, default=2000.0)
    parser.add_argument("--huber-beta", type=float, default=120.0)
    parser.add_argument(
        "--validation-fraction",
        type=float,
        default=0.15,
        help="Deterministic held-out fraction used only for reporting (0 disables it).",
    )
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument(
        "--max-delta",
        type=float,
        default=0.35,
        help="Maximum absolute change from an in-engine term weight (0 disables the cap).",
    )
    parser.add_argument(
        "--allow-draw-scaled",
        action="store_true",
        help="Keep positions where HCE's endgame draw multiplier is active.",
    )
    parser.add_argument(
        "--solver",
        choices=("adam", "ridge"),
        default="adam",
        help="Use ridge closed form for linear centipawn fitting.",
    )
    args = parser.parse_args()

    if not 0.0 <= args.validation_fraction < 1.0:
        raise ValueError("validation-fraction must be in [0, 1)")
    if args.max_delta < 0.0:
        raise ValueError("max-delta must be non-negative")
    if args.l2 < 0.0:
        raise ValueError("l2 must be non-negative")

    header = pd.read_csv(args.input, nrows=0)
    feature_columns = [
        name
        for name in header.columns
        if name not in NON_FEATURE_COLUMNS
    ]
    if set(feature_columns) != set(HCE_FEATURE_NAMES):
        missing_features = set(HCE_FEATURE_NAMES).difference(feature_columns)
        extra_features = set(feature_columns).difference(HCE_FEATURE_NAMES)
        detail = []
        if missing_features:
            detail.append(f"missing: {', '.join(sorted(missing_features))}")
        if extra_features:
            detail.append(f"unexpected: {', '.join(sorted(extra_features))}")
        raise ValueError("CSV feature columns do not match HCE_FEATURE_NAMES (" + "; ".join(detail) + ")")
    # Emit in the Rust evaluator's canonical order even if a CSV was reordered.
    names = list(HCE_FEATURE_NAMES)

    required = names + [args.target, "tempo", "full_white"]
    missing = set(required).difference(header.columns)
    if missing:
        raise ValueError(f"input is missing required columns: {', '.join(sorted(missing))}")
    frame = pd.read_csv(args.input, usecols=required, dtype="float32")

    if args.target == "sf_cp" and "sf_cp" not in frame.columns:
        raise ValueError("target sf_cp requires an sf_cp column")

    if not args.allow_draw_scaled:
        # The engine applies endgame draw scaling *after* summing weighted
        # features and tempo. A linear fit cannot faithfully reproduce that
        # non-linearity, so use the pre-scaling subset by default. The sum of
        # individually tapered features can differ from the aggregate by a few
        # centipawns due to rounding; a 32 cp tolerance keeps those rows.
        unscaled = frame[names].sum(axis=1) + frame["tempo"]
        keep = (frame["full_white"] - unscaled).abs() <= 32.0
        removed = int((~keep).sum())
        frame = frame.loc[keep].reset_index(drop=True)
        if removed:
            print(f"excluded {removed:,} draw-scaled rows")
    if len(frame) < 2:
        raise ValueError("need at least two unscaled rows to fit and validate")

    x = torch.from_numpy(frame[names].to_numpy(copy=True)) / args.scale
    tempo = torch.from_numpy(frame["tempo"].to_numpy(copy=True)) / args.scale
    if args.target == "result":
        y = torch.from_numpy(frame["result"].to_numpy(copy=True))
        fixed_offset = tempo
    else:
        y = torch.from_numpy(
            (frame["sf_cp"].clip(-args.clip_cp, args.clip_cp) - frame["tempo"]).to_numpy(copy=True)
        ) / args.scale
        fixed_offset = torch.zeros_like(tempo)

    weights = torch.nn.Parameter(CURRENT_WEIGHTS.clone())

    def constrain_weights() -> None:
        if args.max_delta > 0.0:
            with torch.no_grad():
                weights.clamp_(CURRENT_WEIGHTS - args.max_delta, CURRENT_WEIGHTS + args.max_delta)

    generator = torch.Generator().manual_seed(args.seed)
    permutation = torch.randperm(x.shape[0], generator=generator)
    validation_rows = int(x.shape[0] * args.validation_fraction)
    if validation_rows and validation_rows == x.shape[0]:
        validation_rows -= 1
    validation_idx = permutation[:validation_rows]
    train_idx = permutation[validation_rows:]
    train_x = x[train_idx]
    train_y = y[train_idx]
    train_offset = fixed_offset[train_idx]
    validation_x = x[validation_idx]
    validation_y = y[validation_idx]
    validation_offset = fixed_offset[validation_idx]

    if args.target == "sf_cp" and args.solver == "ridge":
        # Match Adam's mean-squared penalty: the data error is averaged over
        # rows and the prior error over weights, so the normal equation gets
        # `l2 * rows / weights` rather than an unscaled `l2` diagonal.
        prior_strength = args.l2 * len(train_x) / train_x.shape[1]
        reg = prior_strength * torch.eye(train_x.shape[1], dtype=train_x.dtype)
        lhs = train_x.T @ train_x + reg
        rhs = train_x.T @ train_y + prior_strength * CURRENT_WEIGHTS
        solution = torch.linalg.solve(lhs, rhs)
        with torch.no_grad():
            weights.copy_(solution)
        constrain_weights()
        write_weights(args.output, names, weights.detach())
        print_validation_metric(
            args.target,
            validation_x,
            validation_y,
            validation_offset,
            weights,
            args.scale,
        )
        print(f"fit {len(names)} weights from {len(train_x):,} rows -> {args.output}")
        print_largest_moves(names, weights.detach())
        return

    opt = torch.optim.AdamW([weights], lr=args.lr, weight_decay=0.0)

    for epoch in range(args.epochs):
        opt.zero_grad()
        if args.batch_size > 0 and args.batch_size < train_x.shape[0]:
            idx = torch.randint(train_x.shape[0], (args.batch_size,), generator=generator)
            xb = train_x[idx]
            yb = train_y[idx]
            offsetb = train_offset[idx]
        else:
            xb = train_x
            yb = train_y
            offsetb = train_offset
        logits = xb @ weights + offsetb
        if args.target == "result":
            loss = F.binary_cross_entropy_with_logits(logits, yb)
        else:
            loss = F.smooth_l1_loss(logits, yb, beta=args.huber_beta / args.scale)
        loss = loss + args.l2 * F.mse_loss(weights, CURRENT_WEIGHTS)
        loss.backward()
        opt.step()
        constrain_weights()
        if (epoch + 1) % 500 == 0:
            print(f"epoch {epoch + 1}: loss={loss.item():.6f}", flush=True)

    write_weights(args.output, names, weights.detach())

    print_validation_metric(
        args.target,
        validation_x,
        validation_y,
        validation_offset,
        weights,
        args.scale,
    )
    print(f"fit {len(names)} weights from {len(train_x):,} rows -> {args.output}")
    print_largest_moves(names, weights.detach())


if __name__ == "__main__":
    main()
