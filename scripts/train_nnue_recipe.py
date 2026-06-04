#!/usr/bin/env python3
"""Run a repeatable NNUE training experiment."""

import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path


WAC_RE = re.compile(r"(\d+)/(\d+)")


def run(cmd: list[str]) -> None:
    print("+ " + " ".join(cmd), flush=True)
    subprocess.run(cmd, check=True)


def existing(path: str) -> str:
    if not Path(path).exists():
        raise FileNotFoundError(path)
    return path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", required=True)
    parser.add_argument("--base-data", default="data/elite_ccrl_sf_d10_5m.txt")
    parser.add_argument("--deep-data", default="data/elite_ccrl_sf_d14_100k.txt")
    parser.add_argument("--init-nnue", default="src/board/nnue/nets/default.nnue")
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--sample-base", type=int, default=200_000)
    parser.add_argument("--sample-deep", type=int, default=50_000)
    parser.add_argument("--hce-distill", type=int, default=0)
    parser.add_argument(
        "--hce-only",
        action="store_true",
        help="Train only on HCE-distilled labels; requires --hce-distill > 0.",
    )
    parser.add_argument("--epochs", type=int, default=4)
    parser.add_argument("--batch-size", type=int, default=8192)
    parser.add_argument("--lr", type=float, default=2e-4)
    parser.add_argument("--eval-clip", type=float, default=1200)
    parser.add_argument("--anchor-lambda", type=float, default=1e-4)
    parser.add_argument("--wdl-lambda", type=float, default=0.0)
    parser.add_argument("--workers", type=int, default=0)
    parser.add_argument("--wac-limit", type=int, default=201)
    parser.add_argument("--wac-movetime-ms", type=int, default=500)
    parser.add_argument("--skip-wac", action="store_true")
    parser.add_argument("--freeze-feature", action="store_true")
    parser.add_argument("--freeze-output", action="store_true")
    parser.add_argument("--no-augment", action="store_true")
    parser.add_argument("--raw-eval-loss", action="store_true")
    args = parser.parse_args()

    run_dir = Path("runs") / args.name
    data_dir = Path("data")
    run_dir.mkdir(parents=True, exist_ok=True)
    data_dir.mkdir(parents=True, exist_ok=True)

    base_data = existing(args.base_data)
    deep_data = existing(args.deep_data)
    init_nnue = existing(args.init_nnue)

    mix = data_dir / f"{args.name}_mix.txt"
    mix_cmd = [
        sys.executable,
        "scripts/make_training_mix.py",
        "--output",
        str(mix),
        "--sample",
        base_data,
        str(args.sample_base),
        "--sample",
        deep_data,
        str(args.sample_deep),
    ]
    run(mix_cmd)

    train_inputs = [str(mix)]
    if args.hce_distill > 0:
        hce_data = data_dir / f"{args.name}_hce.txt"
        run(
            [
                sys.executable,
                "scripts/label_positions_static_eval.py",
                "--engine",
                args.engine,
                "--input",
                str(mix),
                "--output",
                str(hce_data),
                "--limit",
                str(args.hce_distill),
                "--setoption",
                "UseNNUE value false",
            ]
        )
        train_inputs.append(str(hce_data))
        if args.hce_only:
            train_inputs = [str(hce_data)]
    elif args.hce_only:
        parser.error("--hce-only requires --hce-distill > 0")

    output = run_dir / f"{args.name}.nnue"
    checkpoint_prefix = run_dir / "checkpoint"
    train_cmd = [
        sys.executable,
        "scripts/train_nnue_improved.py",
        "--data",
        *train_inputs,
        "--output",
        str(output),
        "--init-piece-square-nnue",
        init_nnue,
        "--epochs",
        str(args.epochs),
        "--batch-size",
        str(args.batch_size),
        "--lr",
        str(args.lr),
        "--eval-clip",
        str(args.eval_clip),
        "--anchor-lambda",
        str(args.anchor_lambda),
        "--wdl-lambda",
        str(args.wdl_lambda),
        "--workers",
        str(args.workers),
        "--checkpoint-prefix",
        str(checkpoint_prefix),
        "--checkpoint-dir",
        str(run_dir),
    ]
    if args.freeze_feature:
        train_cmd.append("--freeze-feature")
    if args.freeze_output:
        train_cmd.append("--freeze-output")
    if args.no_augment:
        train_cmd.append("--no-augment")
    if args.raw_eval_loss:
        train_cmd.append("--raw-eval-loss")
    run(train_cmd)

    if not args.skip_wac:
        candidates = sorted(run_dir.glob("checkpoint_epoch_*.nnue")) + [output]
        scores: list[tuple[str, str]] = []
        best_score = -1
        best_candidate: Path | None = None
        for candidate in candidates:
            cmd = [
                sys.executable,
                "scripts/wac_eval_nnue.py",
                "--engine",
                args.engine,
                "--epd",
                "tests/data/wac_all.epd",
                "--limit",
                str(args.wac_limit),
                "--movetime-ms",
                str(args.wac_movetime_ms),
                "--nnue",
                str(candidate),
            ]
            print("+ " + " ".join(cmd), flush=True)
            result = subprocess.run(cmd, check=True, text=True, capture_output=True)
            print(result.stdout, end="", flush=True)
            score = result.stdout.strip()
            scores.append((str(candidate), score))
            if match := WAC_RE.search(score):
                correct = int(match.group(1))
                if correct > best_score:
                    best_score = correct
                    best_candidate = candidate
        print("WAC summary:")
        for candidate, score in scores:
            print(f"  {candidate}: {score}")
        if best_candidate is not None:
            best_wac = run_dir / "best_wac.nnue"
            shutil.copy2(best_candidate, best_wac)
            print(f"Best WAC checkpoint: {best_candidate} -> {best_wac}")


if __name__ == "__main__":
    main()
