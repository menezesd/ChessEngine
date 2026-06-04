#!/usr/bin/env python3
"""Fine-tune NNUE with pairwise legal-child ranking loss."""

import argparse
import random
import sys
from dataclasses import dataclass

import chess
import torch
import torch.nn.functional as F
from torch.utils.data import DataLoader, Dataset

sys.path.insert(0, "scripts")
from train_nnue_improved import MAX_ACTIVE_FEATURES, SCALE, NNUE256, get_device, parse_fen_features


@dataclass
class Pair:
    best_fen: str
    other_fen: str
    weight: float


def parse_epd(line: str):
    board = chess.Board()
    ops = board.set_epd(line.strip())
    best = ops.get("bm", [])
    return board, set(best)


def collect_pairs(
    epd_paths: list[str],
    max_roots: int,
    negatives_per_best: int,
    repeat: int,
    seed: int,
) -> list[Pair]:
    rng = random.Random(seed)
    pairs: list[Pair] = []
    roots = 0

    for path in epd_paths:
        with open(path) as f:
            for line in f:
                if not line.strip():
                    continue
                if max_roots and roots >= max_roots:
                    return pairs
                try:
                    board, best_moves = parse_epd(line)
                except Exception:
                    continue
                roots += 1
                legal = list(board.legal_moves)
                best = [m for m in legal if m in best_moves]
                others = [m for m in legal if m not in best_moves]
                if not best or not others:
                    continue
                for best_move in best:
                    best_board = board.copy(stack=False)
                    best_board.push(best_move)
                    sample_size = min(negatives_per_best, len(others))
                    negatives = rng.sample(others, sample_size)
                    for other_move in negatives:
                        other_board = board.copy(stack=False)
                        other_board.push(other_move)
                        for _ in range(repeat):
                            pairs.append(Pair(best_board.fen(), other_board.fen(), 1.0))
    return pairs


def load_pair_file(path: str, repeat: int) -> list[Pair]:
    pairs: list[Pair] = []
    with open(path) as f:
        for line in f:
            line = line.rstrip("\n")
            if not line or "|" not in line:
                continue
            parts = [part.strip() for part in line.split("|")]
            if len(parts) < 2:
                continue
            best_fen, other_fen = parts[0], parts[1]
            weight = 1.0
            if len(parts) >= 3:
                try:
                    weight = float(parts[2])
                except ValueError:
                    weight = 1.0
            for _ in range(repeat):
                pairs.append(Pair(best_fen, other_fen, weight))
    return pairs


class PairDataset(Dataset):
    def __init__(self, pairs: list[Pair]):
        self.pairs = pairs

    def __len__(self):
        return len(self.pairs)

    @staticmethod
    def tensorize(fen: str):
        white_feat, black_feat, stm = parse_fen_features(fen, do_mirror=False)
        white_tensor = torch.full((MAX_ACTIVE_FEATURES,), -1, dtype=torch.long)
        black_tensor = torch.full((MAX_ACTIVE_FEATURES,), -1, dtype=torch.long)
        white_tensor[: min(len(white_feat), MAX_ACTIVE_FEATURES)] = torch.tensor(
            white_feat[:MAX_ACTIVE_FEATURES], dtype=torch.long
        )
        black_tensor[: min(len(black_feat), MAX_ACTIVE_FEATURES)] = torch.tensor(
            black_feat[:MAX_ACTIVE_FEATURES], dtype=torch.long
        )
        return white_tensor, black_tensor, torch.tensor(stm, dtype=torch.bool)

    def __getitem__(self, idx):
        pair = self.pairs[idx]
        bw, bb, bstm = self.tensorize(pair.best_fen)
        ow, ob, ostm = self.tensorize(pair.other_fen)
        return bw, bb, bstm, ow, ob, ostm, torch.tensor(pair.weight, dtype=torch.float32)


def train_epoch(model, loader, optimizer, device, margin_cp: float):
    model.train()
    total = 0.0
    batches = 0
    margin = margin_cp / SCALE
    for batch in loader:
        bw, bb, bstm, ow, ob, ostm, weight = [x.to(device) for x in batch]
        optimizer.zero_grad()

        # Model output is from side-to-move perspective in the child position.
        # Parent move quality is the negative child STM eval.
        best_parent_score = -model(bw, bb, bstm).squeeze(1)
        other_parent_score = -model(ow, ob, ostm).squeeze(1)
        loss = F.relu(margin - (best_parent_score - other_parent_score))
        loss = (loss * weight).mean()
        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        optimizer.step()

        total += loss.item()
        batches += 1
    return total / max(batches, 1)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--epd", nargs="+", default=[])
    parser.add_argument("--pair-file", action="append", default=[])
    parser.add_argument("--init-nnue")
    parser.add_argument("--init-piece-square-nnue")
    parser.add_argument("--init-from-pst", action="store_true")
    parser.add_argument("--output", required=True)
    parser.add_argument("--checkpoint-prefix", default=None)
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--batch-size", type=int, default=1024)
    parser.add_argument("--lr", type=float, default=1e-6)
    parser.add_argument("--margin-cp", type=float, default=80)
    parser.add_argument("--max-roots", type=int, default=0)
    parser.add_argument("--negatives", type=int, default=8)
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--freeze-feature", action="store_true")
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    pairs = []
    if args.epd:
        pairs.extend(collect_pairs(args.epd, args.max_roots, args.negatives, args.repeat, args.seed))
    for path in args.pair_file:
        pairs.extend(load_pair_file(path, args.repeat))
    random.Random(args.seed).shuffle(pairs)
    print(f"Loaded {len(pairs):,} ranking pairs")

    device = get_device()
    model = NNUE256().to(device)
    if args.init_from_pst:
        model.init_from_pst()
    if args.init_nnue:
        model.load_quantized(args.init_nnue)
    if args.init_piece_square_nnue:
        model.load_piece_square_quantized(args.init_piece_square_nnue)
    if args.freeze_feature:
        model.feature_weights.requires_grad_(False)
        model.feature_bias.requires_grad_(False)
        print("Freezing feature transformer; training output weights only")

    loader = DataLoader(
        PairDataset(pairs),
        batch_size=args.batch_size,
        shuffle=True,
        num_workers=0,
        pin_memory=False,
        drop_last=True,
    )
    optimizer = torch.optim.AdamW(
        [p for p in model.parameters() if p.requires_grad],
        lr=args.lr,
        weight_decay=0.0,
    )

    best_loss = float("inf")
    for epoch in range(args.epochs):
        loss = train_epoch(model, loader, optimizer, device, args.margin_cp)
        print(f"Epoch {epoch + 1}/{args.epochs}: rank_loss={loss:.6f}", flush=True)
        if args.checkpoint_prefix:
            model.export_quantized(f"{args.checkpoint_prefix}_epoch_{epoch + 1}.nnue")
        if loss < best_loss:
            best_loss = loss
            model.export_quantized(args.output)

    print(f"Done. Best rank loss: {best_loss:.6f}")


if __name__ == "__main__":
    main()
