#!/usr/bin/env python3
"""Test a training checkpoint on WAC by exporting to NNUE and running WAC test.

Usage:
    python3 scripts/test_checkpoint_wac.py checkpoint_improved_epoch_5.pt
    python3 scripts/test_checkpoint_wac.py runs/strong_nnue.nnue  # also accepts .nnue files directly
"""
import argparse
from contextlib import ExitStack
import os
import tempfile

import torch

if __package__:
    from .train_nnue_improved import NNUE256
    from .wac_eval_nnue import UCIEngine, iter_epd, parse_epd as _parse_epd, uci_to_san
else:
    from train_nnue_improved import NNUE256
    from wac_eval_nnue import UCIEngine, iter_epd, parse_epd as _parse_epd, uci_to_san


def export_checkpoint_to_nnue(checkpoint_path: str) -> str:
    """Export a .pt checkpoint to a temporary .nnue file."""
    device = torch.device("cpu")
    model = NNUE256().to(device)
    ckpt = torch.load(checkpoint_path, map_location=device)
    model.load_state_dict(ckpt["model_state_dict"])
    model.eval()

    # Close the temporary file before the exporter opens it, including on Windows.
    with tempfile.NamedTemporaryFile(suffix=".nnue", delete=False) as tmp:
        path = tmp.name
    try:
        model.export_quantized(path)
    except BaseException:
        os.unlink(path)
        raise
    return path


def parse_epd(line):
    return _parse_epd(line)[:2]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("checkpoint", help=".pt checkpoint or .nnue file")
    parser.add_argument("--engine", default="./target/release/chess_engine")
    parser.add_argument("--epd", default="tests/data/wac_all.epd")
    parser.add_argument("--limit", type=int, default=201)
    parser.add_argument("--movetime-ms", type=int, default=1000)
    args = parser.parse_args()

    with ExitStack() as cleanup:
        if args.checkpoint.endswith(".pt"):
            print(f"Exporting {args.checkpoint} to NNUE...", flush=True)
            nnue_path = export_checkpoint_to_nnue(args.checkpoint)
            cleanup.callback(os.unlink, nnue_path)
        else:
            nnue_path = args.checkpoint

        engine = UCIEngine(args.engine, nnue_path)
        cleanup.callback(engine.quit)
        total = correct = 0
        for fen, bms, _ in iter_epd(args.epd, args.limit):
            total += 1
            best = engine.bestmove(fen, args.movetime_ms)
            san = uci_to_san(fen, best)
            if san in bms or best in bms:
                correct += 1
            if total % 50 == 0:
                print(f"  Progress: {correct}/{total}", flush=True)
        percent = 100 * correct / total if total else 0
        print(f"\nWAC: {correct}/{total} ({percent:.1f}%)")


if __name__ == "__main__":
    main()
