#!/usr/bin/env python3
"""
Improved NNUE Training Script

Uses horizontal mirroring, grouped validation, an optional game-result loss
and gradient accumulation. Piece features are expanded only for the active
batch so the size of the training set does not dictate the dense tensor size.
"""

import argparse
from array import array
from contextlib import nullcontext
from hashlib import blake2b
import math
import random
import sys
import tempfile
import time
from pathlib import Path
from typing import List, Optional, Tuple

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.utils.data import Dataset, DataLoader, Subset

from train_nnue_256 import MATERIAL_EG, MATERIAL_MG, PST_EG, PST_MG

# Constants matching Rust implementation
INPUT_SIZE = 768  # 64 squares × 6 pieces × 2 colors
HIDDEN_SIZE = 256  # Must match the Rust network and exported file layout.
SCALE = 400
QA = 255
QB = 64
# Target-space cap for forced-win/mate evaluations. Evals beyond eval_clip
# stretch toward this value (in centipawns) so mates stay sharply separated
# from quiet advantages without dominating the loss.
EVAL_TARGET_CAP = 3000
# Typical magnitude of a Stockfish mate score (32000 - ply).
MATE_SCORE_FLOOR = 32000

PIECE_MAP = {
    "P": 0,
    "N": 1,
    "B": 2,
    "R": 3,
    "Q": 4,
    "K": 5,
    "p": 0,
    "n": 1,
    "b": 2,
    "r": 3,
    "q": 4,
    "k": 5,
}


def get_device():
    """Get the best available device."""
    if torch.backends.mps.is_available():
        print("Using MPS (Apple Silicon)")
        return torch.device("mps")
    elif torch.cuda.is_available():
        print("Using CUDA")
        return torch.device("cuda")
    else:
        print("Using CPU")
        return torch.device("cpu")


def get_dataloader_config(device: torch.device) -> dict:
    if sys.platform == "darwin" and device.type == "mps":
        print(
            "Using safe DataLoader settings for macOS/MPS (num_workers=0, pin_memory=False)"
        )
        return {
            "num_workers": 0,
            "pin_memory": False,
        }

    return {
        "num_workers": 4,
        "pin_memory": True,
    }


def feature_index(
    piece_type: int, piece_color: int, square: int, perspective: int
) -> int:
    """Compute feature index for NNUE."""
    index = piece_color * 384 + piece_type * 64 + square
    return _opposite_perspective(index) if perspective == 1 else index


def _opposite_perspective(indices):
    """Swap feature colors and flip ranks for scalar or batched indices."""
    return ((indices + INPUT_SIZE // 2) % INPUT_SIZE) ^ 56


def mirror_square(sq: int) -> int:
    """Mirror square horizontally (a1 <-> h1, etc.)."""
    return sq ^ 7


def parse_fen_features(
    fen: str, do_mirror: bool = False
) -> Tuple[List[int], List[int], bool]:
    """Parse FEN and return feature indices for both perspectives.

    If do_mirror=True, horizontally mirror the position (for data augmentation).
    """
    white, white_to_move = _white_fen_features(fen, do_mirror)
    return white, [_opposite_perspective(index) for index in white], white_to_move


def _white_fen_features(fen: str, do_mirror: bool = False) -> Tuple[List[int], bool]:
    parts = fen.split(maxsplit=2)
    white_to_move = len(parts) == 1 or parts[1] == "w"
    white_features = []
    square = 56

    for char in parts[0]:
        if char == "/":
            square -= 16
        elif char.isdigit():
            square += int(char)
        else:
            piece_type = PIECE_MAP[char]
            piece_color = 0 if char.isupper() else 1

            # Apply horizontal mirror if requested
            actual_sq = mirror_square(square) if do_mirror else square

            white_features.append(feature_index(piece_type, piece_color, actual_sq, 0))
            square += 1

    return white_features, white_to_move


def is_quiet_position(fen: str) -> bool:
    """Legacy material filter for positions with at most two pawns and kings."""
    parts = fen.split()
    board = parts[0]

    # Count pieces (excluding kings)
    piece_counts = {"p": 0, "n": 0, "b": 0, "r": 0, "q": 0}
    for char in board.lower():
        if char in piece_counts:
            piece_counts[char] += 1

    # Skip positions with just pawns and kings (endgame tablebase)
    non_pawn_pieces = sum(piece_counts[p] for p in "nbrq")
    if non_pawn_pieces == 0 and piece_counts["p"] <= 2:
        return False

    return True


class NNUE256(nn.Module):
    """NNUE Network: (768 -> 256) x 2 perspectives -> 1"""

    def __init__(self, dropout: float = 0.0):
        super().__init__()
        self.feature_weights = nn.Parameter(torch.zeros(INPUT_SIZE, HIDDEN_SIZE))
        self.feature_bias = nn.Parameter(torch.zeros(HIDDEN_SIZE))
        self.output_weights_white = nn.Parameter(torch.zeros(HIDDEN_SIZE))
        self.output_weights_black = nn.Parameter(torch.zeros(HIDDEN_SIZE))
        self.output_bias = nn.Parameter(torch.zeros(1))
        self.dropout = nn.Dropout(dropout) if dropout > 0 else None
        self._init_weights()

    def _init_weights(self):
        nn.init.kaiming_normal_(
            self.feature_weights, mode="fan_out", nonlinearity="relu"
        )
        self.feature_weights.data *= 0.18
        nn.init.zeros_(self.feature_bias)
        nn.init.normal_(self.output_weights_white, std=0.06)
        nn.init.normal_(self.output_weights_black, std=0.06)
        nn.init.zeros_(self.output_bias)

    def init_from_pst(self):
        print("Initializing NNUE from HCE piece-square tables...")
        with torch.no_grad():
            self.feature_weights.data.zero_()

            for piece_color in range(2):
                for piece_type in range(6):
                    for square in range(64):
                        feature_idx = piece_color * 384 + piece_type * 64 + square

                        if piece_color == 0:
                            pst_sq = square
                            sign = 1
                        else:
                            pst_sq = square ^ 56
                            sign = -1

                        pst_mg = PST_MG[piece_type][pst_sq]
                        pst_eg = PST_EG[piece_type][pst_sq]
                        material_mg = MATERIAL_MG[piece_type]
                        material_eg = MATERIAL_EG[piece_type]

                        pst_value = 0.7 * pst_mg + 0.3 * pst_eg
                        material_value = 0.7 * material_mg + 0.3 * material_eg

                        if piece_type < 5:
                            total_value = (pst_value + material_value) * sign
                        else:
                            total_value = pst_value * sign

                        scaled_value = total_value / SCALE / (HIDDEN_SIZE**0.5)

                        for h in range(HIDDEN_SIZE):
                            self.feature_weights.data[feature_idx, h] = scaled_value * (
                                0.8 + 0.4 * ((h + piece_type) % 5) / 5
                            )

            # Keep the output roles signed when the side to move swaps.
            self.output_weights_white.data.fill_(1.0 / (HIDDEN_SIZE**0.5))
            self.output_weights_black.data.fill_(-1.0 / (HIDDEN_SIZE**0.5))

        print("  PST initialization complete")

    def forward(
        self,
        white_features: torch.Tensor,
        black_features: torch.Tensor,
        white_to_move: torch.Tensor,
    ) -> torch.Tensor:
        """Forward pass."""
        # Compute accumulators
        white_acc = F.linear(
            white_features, self.feature_weights.t(), self.feature_bias
        )
        black_acc = F.linear(
            black_features, self.feature_weights.t(), self.feature_bias
        )

        # SCReLU activation (clamp to [0, 1], then square)
        white_acc = torch.clamp(white_acc, 0, 1) ** 2
        black_acc = torch.clamp(black_acc, 0, 1) ** 2

        # Apply dropout during training
        if self.dropout and self.training:
            white_acc = self.dropout(white_acc)
            black_acc = self.dropout(black_acc)

        # Select us/them based on side to move
        white_to_move_exp = white_to_move.unsqueeze(1)
        us_acc = torch.where(white_to_move_exp, white_acc, black_acc)
        them_acc = torch.where(white_to_move_exp, black_acc, white_acc)

        # Weights have fixed us/them roles; only accumulators follow the turn.
        us_weights = self.output_weights_white
        them_weights = self.output_weights_black

        output = (
            (us_acc * us_weights).sum(dim=1)
            + (them_acc * them_weights).sum(dim=1)
            + self.output_bias.squeeze()
        )

        return output.unsqueeze(1)

    def export_quantized(self, path: str):
        """Export to quantized binary format for Rust."""
        parts = []
        weights = (
            ("feature_weights", QA),
            ("feature_bias", QA),
            ("output_weights_white", QB),
            ("output_weights_black", QB),
            ("output_bias", QA * QB),
        )
        for name, scale in weights:
            values = getattr(self, name).detach().cpu().numpy()
            if not np.isfinite(values).all():
                raise ValueError(f"Cannot export non-finite parameter: {name}")
            parts.append((values * scale).round().clip(-32768, 32767).astype("<i2").tobytes())
        payload = b"".join(parts)

        # An engine loading the file during training must see a complete network.
        destination = Path(path).resolve()
        with tempfile.TemporaryDirectory(dir=destination.parent) as directory:
            temporary = Path(directory) / "weights.nnue"
            temporary.write_bytes(payload)
            temporary.replace(destination)
        print(f"Exported network to {path} ({len(payload)} bytes)")


class ImprovedChessDataset(Dataset):
    """Store one sparse row per position and expand only active batches."""

    MAX_PIECES = 32

    def __init__(
        self,
        data_files: List[str],
        max_positions: Optional[int] = None,
        filter_quiet: bool = True,
        augment: bool = True,
        eval_clip: float = 1500,
        precompute: bool = True,
    ):
        self.augment = augment
        self.precompute = precompute
        self.eval_clip = eval_clip
        features = array("h")
        scores = array("f")
        turns = array("B")
        results = array("f")
        groups = array("Q")
        skipped = 0

        for data_file in data_files:
            print(f"Loading {data_file}...")
            with open(data_file) as source:
                for line in source:
                    if max_positions and len(scores) >= max_positions:
                        break
                    parts = line.split("|")
                    if len(parts) < 2:
                        continue
                    fen = parts[0].strip()
                    try:
                        score = float(parts[1])
                        if not math.isfinite(score) or (filter_quiet and not is_quiet_position(fen)):
                            skipped += 1
                            continue
                        white, white_to_move = _white_fen_features(fen)
                    except (IndexError, ValueError, KeyError):
                        skipped += 1
                        continue
                    if len(white) > self.MAX_PIECES or any(
                        not 0 <= index < INPUT_SIZE for index in white
                    ):
                        skipped += 1
                        continue

                    game_result = 0.5
                    if len(parts) >= 3:
                        if parts[2].strip() in ("1.0", "1"):
                            game_result = 1.0
                        elif parts[2].strip() in ("0.0", "0"):
                            game_result = 0.0

                    features.extend(white + [INPUT_SIZE] * (self.MAX_PIECES - len(white)))
                    turns.append(white_to_move)
                    scores.append(self._scale_eval_target(score if white_to_move else -score))
                    results.append(game_result if white_to_move else 1.0 - game_result)
                    groups.append(self._group_key(fen, white_to_move))

            if max_positions and len(scores) >= max_positions:
                break

        self.positions = torch.from_numpy(np.frombuffer(features, dtype=np.int16)).view(
            len(scores), self.MAX_PIECES
        )
        self.scores = torch.from_numpy(np.frombuffer(scores, dtype=np.float32)).reshape(-1, 1)
        self.turns = torch.from_numpy(np.frombuffer(turns, dtype=np.bool_))
        self.results = torch.from_numpy(np.frombuffer(results, dtype=np.float32)).reshape(-1, 1)
        self.groups = np.frombuffer(groups, dtype=np.uint64)

        print(f"Loaded {len(self.positions):,} positions (skipped {skipped:,})")
        if augment:
            print(f"With augmentation: {len(self):,} effective positions")

    @staticmethod
    def _group_key(fen: str, white_to_move: bool) -> int:
        # Counters and rights are absent from the NNUE inputs. Group a board
        # with its horizontal mirror even if both appear in the source data.
        board = fen.split(maxsplit=1)[0]
        mirrored = "/".join(rank[::-1] for rank in board.split("/"))
        key = min(board, mirrored) + (" w" if white_to_move else " b")
        return int.from_bytes(blake2b(key.encode(), digest_size=8).digest(), "little")

    def _scale_eval_target(self, eval_score: float) -> float:
        """Keep mate targets above quieter advantages while bounding their loss."""
        x = min(abs(eval_score), MATE_SCORE_FLOOR)
        if x <= self.eval_clip:
            return math.copysign(x / SCALE, eval_score)
        slope = (EVAL_TARGET_CAP - self.eval_clip) / (MATE_SCORE_FLOOR - self.eval_clip)
        capped = self.eval_clip + slope * (x - self.eval_clip)
        return math.copysign(capped / SCALE, eval_score)

    def __len__(self):
        return len(self.positions) * (2 if self.augment else 1)

    def __getitem__(self, idx):
        if self.precompute:
            return int(idx)
        return tuple(value[0] for value in self.batch([idx]))

    def batch(self, indices):
        count = len(self.positions)
        indices = torch.as_tensor(indices, dtype=torch.long)
        actual = indices.remainder(count)
        white = self.positions[actual].long()
        padding = white == INPUT_SIZE
        if self.augment:
            mirror = ((indices >= count) * 7).unsqueeze(1)
            white = torch.where(padding, white, white ^ mirror)
        black = torch.where(padding, white, _opposite_perspective(white))

        # The extra column absorbs padding without overwriting real features.
        def dense(sparse):
            batch = torch.zeros((len(indices), INPUT_SIZE + 1), dtype=torch.float32)
            return batch.scatter_(1, sparse, 1)[:, :INPUT_SIZE]

        return dense(white), dense(black), self.turns[actual], self.scores[actual], self.results[actual]


class BatchIndexCollate:
    def __init__(self, dataset):
        self.dataset = dataset

    def __call__(self, batch):
        return self.dataset.batch(batch)


def collate_dense(batch):
    return tuple(torch.stack(field) for field in zip(*batch))


def split_dataset(dataset, val_split, seed=42):
    """Keep identical inputs and their mirrors in a single split."""
    if not 0.0 <= val_split < 1.0:
        raise ValueError("val_split must be between 0 (inclusive) and 1 (exclusive)")
    count = len(dataset.positions)
    if not count:
        raise ValueError("no usable training positions")
    if not val_split:
        return Subset(dataset, range(len(dataset))), Subset(dataset, range(0))

    # A reproducible hash partition avoids a Python object for every FEN.
    keys = dataset.groups
    salt = np.uint64(random.Random(seed).getrandbits(64))
    validation = (keys ^ salt) < int(val_split * (1 << 64))
    if not validation.any() or validation.all():
        # Retain both splits for small datasets if two groups exist.
        validation = keys != keys[0] if validation.all() else keys == keys[0]
        if validation.all():
            validation[:] = False

    index_type = np.int32 if len(dataset) <= np.iinfo(np.int32).max else np.int64

    def part(mask):
        indices = np.flatnonzero(mask).astype(index_type, copy=False)
        if dataset.augment:
            indices = np.concatenate((indices, indices + count))
        return Subset(dataset, indices)

    return part(~validation), part(validation)


class ShuffledBatches:
    """Shuffle compact indices without making one Python list per epoch."""

    def __init__(self, count, batch_size):
        self.count = count
        self.batch_size = batch_size
        self.dtype = torch.int32 if count <= np.iinfo(np.int32).max else torch.int64

    def __iter__(self):
        for batch in torch.randperm(self.count, dtype=self.dtype).split(self.batch_size):
            yield batch.tolist()

    def __len__(self):
        return (self.count + self.batch_size - 1) // self.batch_size


def _training_losses(
    model, batch, wdl_lambda, raw_eval_loss, use_huber, anchors, anchor_lambda,
):
    white_f, black_f, stm, target, result = batch
    output = model(white_f, black_f, stm)
    pred_wdl = torch.sigmoid(output)
    if raw_eval_loss:
        eval_loss = (
            F.smooth_l1_loss(output, target) if use_huber else F.mse_loss(output, target)
        )
    else:
        eval_loss = F.mse_loss(pred_wdl, torch.sigmoid(target))
    wdl_loss = F.mse_loss(pred_wdl, result) * wdl_lambda
    loss = eval_loss + wdl_loss
    if anchors and anchor_lambda > 0.0:
        for name, param in model.named_parameters():
            if param.requires_grad and name in anchors:
                loss = loss + anchor_lambda * F.mse_loss(param, anchors[name])
    return loss, eval_loss, wdl_loss


def train_epoch(
    model, dataloader, optimizer, device, wdl_lambda=0.25, accumulation_steps=1,
    scaler=None, raw_eval_loss=True, use_huber=True, anchors=None, anchor_lambda=0.0,
):
    """Train on every sample, including partial accumulation groups."""
    if accumulation_steps < 1:
        raise ValueError("accumulation_steps must be positive")
    model.train()
    total_loss = total_eval_loss = total_wdl_loss = 0.0
    n_samples = group_samples = 0
    optimizer.zero_grad()

    def step(samples):
        if scaler is not None:
            scaler.unscale_(optimizer)
        # Accumulated losses are sums, so normalize by actual sample count.
        # This also handles a smaller final batch without overweighting it.
        for param in model.parameters():
            if param.grad is not None:
                param.grad.div_(samples)
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        if scaler is None:
            optimizer.step()
        else:
            scaler.step(optimizer)
            scaler.update()
        optimizer.zero_grad()

    for batch_idx, batch in enumerate(dataloader):
        batch = [x.to(device) for x in batch]
        samples = len(batch[0])
        context = torch.cuda.amp.autocast() if scaler is not None else nullcontext()
        with context:
            loss, eval_loss, wdl_loss = _training_losses(
                model, batch, wdl_lambda, raw_eval_loss, use_huber, anchors, anchor_lambda,
            )
        summed_loss = loss * samples
        if scaler is None:
            summed_loss.backward()
        else:
            scaler.scale(summed_loss).backward()
        group_samples += samples
        if (batch_idx + 1) % accumulation_steps == 0:
            step(group_samples)
            group_samples = 0

        total_loss += loss.item() * samples
        total_eval_loss += eval_loss.item() * samples
        total_wdl_loss += wdl_loss.item() * samples
        n_samples += samples
        if (batch_idx + 1) % 100 == 0:
            print(
                f"  Batch {batch_idx + 1}: loss={total_loss / n_samples:.6f} "
                f"(eval={total_eval_loss / n_samples:.6f}, wdl={total_wdl_loss / n_samples:.6f})",
                flush=True,
            )

    if group_samples:
        step(group_samples)
    if not n_samples:
        raise ValueError("training dataset contains no samples")
    return total_loss / n_samples


def validate(
    model, dataloader, device, wdl_lambda=0.25, raw_eval_loss=True, use_huber=True,
    anchors=None, anchor_lambda=0.0,
):
    """Return loss per sample, independent of validation batch sizes."""
    model.eval()
    total_loss = 0.0
    n_samples = 0
    with torch.no_grad():
        for batch in dataloader:
            batch = [x.to(device) for x in batch]
            loss, _, _ = _training_losses(
                model, batch, wdl_lambda, raw_eval_loss, use_huber, anchors, anchor_lambda,
            )
            samples = len(batch[0])
            total_loss += loss.item() * samples
            n_samples += samples
    if not n_samples:
        raise ValueError("validation dataset contains no samples")
    return total_loss / n_samples


def main():
    parser = argparse.ArgumentParser(description="Improved NNUE Training")
    parser.add_argument("--data", type=str, nargs="+", required=True)
    parser.add_argument("--output", type=str, default="trained_improved.nnue")
    parser.add_argument("--epochs", type=int, default=30)
    parser.add_argument("--batch-size", type=int, default=8192)
    parser.add_argument("--lr", type=float, default=0.001)
    parser.add_argument(
        "--wdl-lambda",
        type=float,
        default=0.0,
        help="WDL loss weight (0 = pure eval, higher adds game result signal)",
    )
    parser.add_argument(
        "--sigmoid-eval-loss",
        action="store_true",
        help="Use legacy sigmoid-space eval loss instead of raw-output regression",
    )
    parser.add_argument(
        "--eval-loss",
        choices=["huber", "mse"],
        default="huber",
        help="Eval regression loss for raw-output training",
    )
    parser.add_argument(
        "--init-from-pst",
        action="store_true",
        help="Initialize NNUE weights from HCE piece-square tables",
    )
    parser.add_argument("--max-positions", type=int, default=None)
    parser.add_argument("--checkpoint", type=str, default=None)
    parser.add_argument(
        "--no-augment",
        action="store_true",
        help="Disable horizontal mirror augmentation",
    )
    parser.add_argument(
        "--no-filter",
        action="store_true",
        help="Include positions with only kings and up to two pawns",
    )
    parser.add_argument(
        "--eval-clip",
        type=float,
        default=1500,
        help="Evals beyond this map linearly toward EVAL_TARGET_CAP instead of "
        "being dropped, so forced wins stay learnable",
    )
    parser.add_argument(
        "--dropout", type=float, default=0.0, help="Dropout rate for regularization"
    )
    parser.add_argument(
        "--val-split", type=float, default=0.05, help="Validation split ratio"
    )
    parser.add_argument(
        "--precompute",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Expand sparse feature rows together when assembling a batch",
    )
    args = parser.parse_args()

    device = get_device()
    model = NNUE256(dropout=args.dropout).to(device)

    if args.init_from_pst and not args.checkpoint:
        model.init_from_pst()

    if args.checkpoint:
        print(f"Loading checkpoint: {args.checkpoint}")
        ckpt = torch.load(args.checkpoint, map_location=device)
        model.load_state_dict(ckpt["model_state_dict"])

    total_params = sum(p.numel() for p in model.parameters())
    print(f"Model parameters: {total_params:,}")

    # Load dataset
    full_dataset = ImprovedChessDataset(
        args.data,
        max_positions=args.max_positions,
        filter_quiet=not args.no_filter,
        augment=not args.no_augment,
        eval_clip=args.eval_clip,
        precompute=args.precompute,
    )

    train_dataset, val_dataset = split_dataset(full_dataset, args.val_split)

    print(f"Train: {len(train_dataset):,}, Val: {len(val_dataset):,}")

    loader_config = get_dataloader_config(device)

    train_loader = DataLoader(
        train_dataset,
        batch_sampler=ShuffledBatches(len(train_dataset), args.batch_size),
        collate_fn=(
            BatchIndexCollate(full_dataset) if args.precompute else collate_dense
        ),
        **loader_config,
    )
    val_loader = DataLoader(
        val_dataset, batch_size=args.batch_size, shuffle=False,
        collate_fn=(
            BatchIndexCollate(full_dataset) if args.precompute else collate_dense
        ),
        **loader_config,
    )

    optimizer = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=0.01)

    # Warmup + cosine annealing schedule
    warmup_epochs = min(3, args.epochs)

    def lr_lambda(epoch):
        if epoch < warmup_epochs:
            return (epoch + 1) / max(1, warmup_epochs)

        anneal_epochs = args.epochs - warmup_epochs
        if anneal_epochs <= 0:
            return 1.0

        progress = (epoch - warmup_epochs) / anneal_epochs
        return 0.01 + 0.99 * 0.5 * (
            1 + torch.cos(torch.tensor(progress * 3.14159)).item()
        )

    scheduler = torch.optim.lr_scheduler.LambdaLR(optimizer, lr_lambda)

    # Mixed precision training (CUDA only)
    scaler = torch.cuda.amp.GradScaler() if device.type == "cuda" else None

    print(f"\nTraining: {args.epochs} epochs, batch={args.batch_size}, lr={args.lr}")
    print(f"WDL lambda: {args.wdl_lambda}, Eval clip: {args.eval_clip}")
    print(
        f"Eval objective: {'sigmoid-mse' if args.sigmoid_eval_loss else f'raw-{args.eval_loss}'}"
    )

    best_val_loss = float("inf")
    patience = 5
    patience_counter = 0

    for epoch in range(args.epochs):
        t0 = time.time()
        train_loss = train_epoch(
            model,
            train_loader,
            optimizer,
            device,
            args.wdl_lambda,
            scaler=scaler,
            raw_eval_loss=not args.sigmoid_eval_loss,
            use_huber=args.eval_loss == "huber",
        )
        val_loss = validate(
            model,
            val_loader,
            device,
            args.wdl_lambda,
            raw_eval_loss=not args.sigmoid_eval_loss,
            use_huber=args.eval_loss == "huber",
        ) if len(val_dataset) else None
        selection_loss = train_loss if val_loss is None else val_loss
        validation_label = "disabled" if val_loss is None else f"{val_loss:.6f}"
        scheduler.step()
        elapsed = time.time() - t0

        print(
            f"Epoch {epoch + 1}/{args.epochs}: "
            f"train={train_loss:.6f}, val={validation_label}, "
            f"lr={optimizer.param_groups[0]['lr']:.6f}, time={elapsed:.1f}s"
        )

        # Save checkpoint
        torch.save(
            {
                "epoch": epoch + 1,
                "model_state_dict": model.state_dict(),
                "train_loss": train_loss,
                "val_loss": val_loss,
            },
            f"checkpoint_improved_epoch_{epoch + 1}.pt",
        )

        # Save best model
        if selection_loss < best_val_loss:
            best_val_loss = selection_loss
            model.export_quantized(args.output)
            print(f"  -> New best! Saved to {args.output}")
            patience_counter = 0
        else:
            patience_counter += 1
            if patience_counter >= patience:
                print(f"Early stopping: no improvement for {patience} epochs")
                break

    metric = "validation" if len(val_dataset) else "training"
    print(f"\nDone! Best {metric} loss: {best_val_loss:.6f}")


if __name__ == "__main__":
    main()
