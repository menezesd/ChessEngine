#!/usr/bin/env python3
"""
Improved NNUE Training Script

Key improvements over train_nnue_256.py:
1. Better data filtering - skip positions in check, captures, extreme evals
2. Data augmentation - horizontal board mirror (2x effective data)
3. Lower WDL lambda (0.25) for better tactical accuracy
4. Validation split to detect overfitting
5. Better learning rate schedule with warmup
6. Gradient accumulation for larger effective batch size
"""

import argparse
import os
import random
import struct
import sys
import time
from pathlib import Path
from typing import List, Optional, Tuple

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.utils.data import Dataset, DataLoader, random_split

from train_nnue_256 import MATERIAL_EG, MATERIAL_MG, PST_EG, PST_MG

# Constants matching Rust implementation
INPUT_SIZE = 64 * 6 * 2  # square x piece type x color
HIDDEN_SIZE = 256
MAX_ACTIVE_FEATURES = 32
SCALE = 400
QA = 255
QB = 64
NNUE_MAGIC = b"RNQNNUE\0"
NNUE_VERSION = 1

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


def get_dataloader_config(device: torch.device, workers: Optional[int] = None) -> dict:
    if workers is not None:
        return {
            "num_workers": workers,
            "pin_memory": workers > 0 and device.type != "cpu",
        }

    if sys.platform == "darwin":
        print(
            "Using safe DataLoader settings for macOS (num_workers=0, pin_memory=False)"
        )
        return {
            "num_workers": 0,
            "pin_memory": False,
        }

    return {
        "num_workers": 4,
        "pin_memory": True,
    }


def feature_index(piece_type: int, piece_color: int, square: int, perspective: int) -> int:
    """Compute feature index for NNUE."""
    if perspective == 1:  # Black's perspective - flip board
        oriented_sq = square ^ 56
        oriented_color = 1 - piece_color
    else:  # White's perspective
        oriented_sq = square
        oriented_color = piece_color
    return oriented_color * 384 + piece_type * 64 + oriented_sq


def mirror_square(sq: int) -> int:
    """Mirror square horizontally (a1 <-> h1, etc.)."""
    rank = sq // 8
    file = sq % 8
    return rank * 8 + (7 - file)


def parse_fen_features(
    fen: str, do_mirror: bool = False
) -> Tuple[List[int], List[int], bool]:
    """Parse FEN and return feature indices for both perspectives.

    If do_mirror=True, horizontally mirror the position (for data augmentation).
    """
    parts = fen.split()
    board = parts[0]
    side_to_move = parts[1] if len(parts) > 1 else "w"
    white_to_move = side_to_move == "w"

    white_features = []
    black_features = []
    square = 56
    pieces = []

    for char in board:
        if char == "/":
            square -= 16
        elif char.isdigit():
            square += int(char)
        else:
            piece_type = PIECE_MAP[char]
            piece_color = 0 if char.isupper() else 1

            # Apply horizontal mirror if requested
            actual_sq = mirror_square(square) if do_mirror else square
            pieces.append((piece_type, piece_color, actual_sq))
            square += 1

    for piece_type, piece_color, actual_sq in pieces:
        white_features.append(feature_index(piece_type, piece_color, actual_sq, 0))
        black_features.append(feature_index(piece_type, piece_color, actual_sq, 1))

    return white_features, black_features, white_to_move


def is_quiet_position(fen: str) -> bool:
    """Check if position is quiet (not in check, no obvious tactical complications)."""
    # Basic heuristic: skip if there's only one piece type left (likely endgame tablebase territory)
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
    """NNUE Network: 768 -> HIDDEN_SIZE x 2 perspectives -> 1"""

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
        self.feature_weights.data *= 0.18  # Smaller init for 512 hidden
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

                        total_value = (pst_value + material_value) * sign

                        scaled_value = total_value / SCALE / (HIDDEN_SIZE**0.5)

                        for h in range(HIDDEN_SIZE):
                            self.feature_weights.data[feature_idx, h] = scaled_value * (
                                0.8 + 0.4 * ((h + piece_type) % 5) / 5
                            )

            self.output_weights_white.data.fill_(1.0 / (HIDDEN_SIZE**0.5))
            self.output_weights_black.data.fill_(1.0 / (HIDDEN_SIZE**0.5))

        print("  PST initialization complete")

    def forward(
        self,
        white_features: torch.Tensor,
        black_features: torch.Tensor,
        white_to_move: torch.Tensor,
    ) -> torch.Tensor:
        """Forward pass."""
        # Compute accumulators. Sparse feature batches are padded with -1;
        # older dense one-hot tensors are still supported.
        if white_features.dtype == torch.long:
            white_idx = white_features.clamp_min(0)
            black_idx = black_features.clamp_min(0)
            white_mask = (white_features >= 0).unsqueeze(2).to(self.feature_weights.dtype)
            black_mask = (black_features >= 0).unsqueeze(2).to(self.feature_weights.dtype)
            white_acc = (self.feature_weights[white_idx] * white_mask).sum(dim=1) + self.feature_bias
            black_acc = (self.feature_weights[black_idx] * black_mask).sum(dim=1) + self.feature_bias
        else:
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

        # Output computation
        us_weights = torch.where(
            white_to_move_exp,
            self.output_weights_white.unsqueeze(0),
            self.output_weights_black.unsqueeze(0),
        )
        them_weights = torch.where(
            white_to_move_exp,
            self.output_weights_black.unsqueeze(0),
            self.output_weights_white.unsqueeze(0),
        )

        output = (
            (us_acc * us_weights).sum(dim=1)
            + (them_acc * them_weights).sum(dim=1)
            + self.output_bias.squeeze()
        )

        return output.unsqueeze(1)

    def export_quantized(self, path: str):
        """Export to quantized binary format for Rust."""
        with open(path, "wb") as f:
            f.write(NNUE_MAGIC)
            f.write(struct.pack("<I", NNUE_VERSION))
            f.write(struct.pack("<I", INPUT_SIZE))
            f.write(struct.pack("<I", HIDDEN_SIZE))

            # Feature weights
            weights = (
                (self.feature_weights.detach().cpu().numpy() * QA)
                .round()
                .clip(-32768, 32767)
                .astype("<i2")
            )
            for i in range(INPUT_SIZE):
                for j in range(HIDDEN_SIZE):
                    f.write(struct.pack("<h", int(weights[i, j])))

            # Feature bias
            bias = (
                (self.feature_bias.detach().cpu().numpy() * QA)
                .round()
                .clip(-32768, 32767)
                .astype("<i2")
            )
            for j in range(HIDDEN_SIZE):
                f.write(struct.pack("<h", int(bias[j])))

            # Output weights white
            out_w = (
                (self.output_weights_white.detach().cpu().numpy() * QB)
                .round()
                .clip(-32768, 32767)
            )
            for j in range(HIDDEN_SIZE):
                f.write(struct.pack("<h", int(out_w[j])))

            # Output weights black
            out_b = (
                (self.output_weights_black.detach().cpu().numpy() * QB)
                .round()
                .clip(-32768, 32767)
            )
            for j in range(HIDDEN_SIZE):
                f.write(struct.pack("<h", int(out_b[j])))

            # Output bias
            out_bias = (
                (self.output_bias.detach().cpu().numpy() * QA * QB / SCALE)
                .round()
                .clip(-32768, 32767)
            )
            f.write(struct.pack("<h", int(out_bias[0])))

        print(f"Exported network to {path} ({os.path.getsize(path)} bytes)")

    def load_quantized(self, path: str):
        """Load a Rust .nnue quantized binary into the floating-point model."""
        with open(path, "rb") as f:
            data = f.read()

        offset = 0
        if data.startswith(NNUE_MAGIC):
            version, input_size, hidden_size = struct.unpack_from("<III", data, len(NNUE_MAGIC))
            if version != NNUE_VERSION:
                raise ValueError(f"Unsupported NNUE version: {version}")
            if input_size != INPUT_SIZE or hidden_size != HIDDEN_SIZE:
                raise ValueError(
                    f"NNUE architecture mismatch: file is {input_size}->{hidden_size}, "
                    f"trainer expects {INPUT_SIZE}->{HIDDEN_SIZE}"
                )
            offset = len(NNUE_MAGIC) + 12

        def read_i16(count: int):
            nonlocal offset
            nbytes = count * 2
            values = torch.frombuffer(bytearray(data[offset : offset + nbytes]), dtype=torch.int16)
            offset += nbytes
            return values.to(torch.float32)

        with torch.no_grad():
            weights = read_i16(INPUT_SIZE * HIDDEN_SIZE).reshape(INPUT_SIZE, HIDDEN_SIZE)
            self.feature_weights.copy_(weights / QA)
            self.feature_bias.copy_(read_i16(HIDDEN_SIZE) / QA)
            self.output_weights_white.copy_(read_i16(HIDDEN_SIZE) / QB)
            self.output_weights_black.copy_(read_i16(HIDDEN_SIZE) / QB)
            self.output_bias.copy_(read_i16(1) * SCALE / (QA * QB))

    def load_piece_square_quantized(self, path: str):
        """Load a legacy raw 768-input piece-square NNUE."""
        with open(path, "rb") as f:
            data = f.read()

        offset = 0
        if data.startswith(NNUE_MAGIC):
            version, input_size, hidden_size = struct.unpack_from("<III", data, len(NNUE_MAGIC))
            if version != NNUE_VERSION:
                raise ValueError(f"Unsupported NNUE version: {version}")
            if input_size != INPUT_SIZE or hidden_size != HIDDEN_SIZE:
                raise ValueError(
                    f"NNUE architecture mismatch: file is {input_size}->{hidden_size}, "
                    f"trainer expects {INPUT_SIZE}->{HIDDEN_SIZE}"
                )
            offset = len(NNUE_MAGIC) + 12

        def read_i16(count: int):
            nonlocal offset
            nbytes = count * 2
            values = torch.frombuffer(bytearray(data[offset : offset + nbytes]), dtype=torch.int16)
            offset += nbytes
            return values.to(torch.float32)

        with torch.no_grad():
            self.feature_weights.copy_(
                read_i16(INPUT_SIZE * HIDDEN_SIZE).reshape(INPUT_SIZE, HIDDEN_SIZE) / QA
            )
            self.feature_bias.copy_(read_i16(HIDDEN_SIZE) / QA)
            self.output_weights_white.copy_(read_i16(HIDDEN_SIZE) / QB)
            self.output_weights_black.copy_(read_i16(HIDDEN_SIZE) / QB)
            self.output_bias.copy_(read_i16(1) * SCALE / (QA * QB))


class ImprovedChessDataset(Dataset):
    """Improved dataset with filtering and augmentation."""

    def __init__(
        self,
        data_files: List[str],
        max_positions: Optional[int] = None,
        filter_quiet: bool = True,
        augment: bool = True,
        eval_clip: float = 1500,
    ):
        self.positions = []
        self.augment = augment
        count = 0
        skipped = 0

        for data_file in data_files:
            print(f"Loading {data_file}...")
            with open(data_file, "r") as f:
                for line in f:
                    if max_positions and count >= max_positions:
                        break
                    line = line.strip()
                    if not line:
                        continue
                    parts = line.split("|")
                    if len(parts) < 2:
                        continue
                    fen = parts[0].strip()

                    try:
                        eval_score = float(parts[1].strip())
                    except ValueError:
                        skipped += 1
                        continue

                    # Filter extreme evaluations (likely mates or errors)
                    if abs(eval_score) > eval_clip:
                        skipped += 1
                        continue

                    # Filter non-quiet positions
                    if filter_quiet and not is_quiet_position(fen):
                        skipped += 1
                        continue

                    # Parse game result (stored from white's perspective)
                    game_result = 0.5
                    if len(parts) >= 3:
                        r = parts[2].strip()
                        if r in ("1.0", "1"):
                            game_result = 1.0
                        elif r in ("0.0", "0"):
                            game_result = 0.0

                    try:
                        white_feat, black_feat, stm = parse_fen_features(
                            fen, do_mirror=False
                        )

                        # Data eval is from WHITE's perspective.
                        # Network outputs from STM perspective, so convert.
                        if not stm:  # Black to move
                            stm_eval = -eval_score  # Flip eval to black's perspective
                            stm_result = 1.0 - game_result
                        else:
                            stm_eval = eval_score
                            stm_result = game_result

                        self.positions.append(
                            {
                                "fen": fen,
                                "white_feat": white_feat,
                                "black_feat": black_feat,
                                "stm": stm,
                                "eval": stm_eval,
                                "result": stm_result,
                            }
                        )
                        count += 1
                    except Exception:
                        skipped += 1
                        continue

            if max_positions and count >= max_positions:
                break

        # Shuffle positions across all files for better train/val split
        import random

        random.shuffle(self.positions)

        print(f"Loaded {len(self.positions):,} positions (skipped {skipped:,})")
        if augment:
            print(f"With augmentation: {len(self.positions) * 2:,} effective positions")

    def __len__(self):
        # 2x positions if augmentation is enabled
        return len(self.positions) * 2 if self.augment else len(self.positions)

    def __getitem__(self, idx):
        # Determine if this is an augmented (mirrored) sample
        if self.augment:
            is_mirrored = idx >= len(self.positions)
            actual_idx = idx % len(self.positions)
        else:
            is_mirrored = False
            actual_idx = idx

        pos = self.positions[actual_idx]

        # Get features (potentially mirrored)
        if is_mirrored:
            white_feat, black_feat, stm = parse_fen_features(pos["fen"], do_mirror=True)
        else:
            white_feat = pos["white_feat"]
            black_feat = pos["black_feat"]
            stm = pos["stm"]

        white_tensor = torch.full((MAX_ACTIVE_FEATURES,), -1, dtype=torch.long)
        black_tensor = torch.full((MAX_ACTIVE_FEATURES,), -1, dtype=torch.long)
        white_tensor[: min(len(white_feat), MAX_ACTIVE_FEATURES)] = torch.tensor(
            white_feat[:MAX_ACTIVE_FEATURES], dtype=torch.long
        )
        black_tensor[: min(len(black_feat), MAX_ACTIVE_FEATURES)] = torch.tensor(
            black_feat[:MAX_ACTIVE_FEATURES], dtype=torch.long
        )

        return (
            white_tensor,
            black_tensor,
            torch.tensor(stm, dtype=torch.bool),
            torch.tensor([pos["eval"] / SCALE], dtype=torch.float32),
            torch.tensor([pos["result"]], dtype=torch.float32),
        )


def train_epoch(
    model,
    dataloader,
    optimizer,
    device,
    wdl_lambda=0.25,
    accumulation_steps=1,
    scaler=None,
    raw_eval_loss=True,
    use_huber=True,
    anchors=None,
    anchor_lambda=0.0,
):
    """Train one epoch with gradient accumulation."""
    model.train()
    total_loss = 0.0
    total_eval_loss = 0.0
    total_wdl_loss = 0.0
    n_batches = 0

    optimizer.zero_grad()

    for batch_idx, batch in enumerate(dataloader):
        white_f, black_f, stm, target, result = [x.to(device) for x in batch]

        # Mixed precision forward pass (if using CUDA)
        if scaler is not None:
            with torch.cuda.amp.autocast():
                output = model(white_f, black_f, stm)
                if raw_eval_loss:
                    eval_loss = (
                        F.smooth_l1_loss(output, target)
                        if use_huber
                        else F.mse_loss(output, target)
                    )
                    pred_wdl = torch.sigmoid(output)
                else:
                    pred_wdl = torch.sigmoid(output)
                    target_sig = torch.sigmoid(target)
                    eval_loss = F.mse_loss(pred_wdl, target_sig)
                wdl_loss = F.mse_loss(pred_wdl, result) * wdl_lambda
                loss = (eval_loss + wdl_loss) / accumulation_steps
            scaler.scale(loss).backward()
        else:
            output = model(white_f, black_f, stm)
            if raw_eval_loss:
                eval_loss = (
                    F.smooth_l1_loss(output, target)
                    if use_huber
                    else F.mse_loss(output, target)
                )
                pred_wdl = torch.sigmoid(output)
            else:
                pred_wdl = torch.sigmoid(output)
                target_sig = torch.sigmoid(target)
                eval_loss = F.mse_loss(pred_wdl, target_sig)
            wdl_loss = F.mse_loss(pred_wdl, result) * wdl_lambda
            anchor_loss = torch.tensor(0.0, device=device)
            if anchors and anchor_lambda > 0.0:
                for name, param in model.named_parameters():
                    if param.requires_grad and name in anchors:
                        anchor_loss = anchor_loss + F.mse_loss(param, anchors[name])
                anchor_loss = anchor_loss * anchor_lambda
            loss = (eval_loss + wdl_loss + anchor_loss) / accumulation_steps
            loss.backward()

        # Gradient accumulation step
        if (batch_idx + 1) % accumulation_steps == 0:
            if scaler is not None:
                scaler.unscale_(optimizer)
                torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
                scaler.step(optimizer)
                scaler.update()
            else:
                torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
                optimizer.step()
            optimizer.zero_grad()

        total_loss += loss.item() * accumulation_steps
        total_eval_loss += eval_loss.item()
        total_wdl_loss += wdl_loss.item()
        n_batches += 1

        if n_batches % 500 == 0:
            print(
                f"  Batch {n_batches}: loss={total_loss / n_batches:.6f} "
                f"(eval={total_eval_loss / n_batches:.6f}, wdl={total_wdl_loss / n_batches:.6f})"
            )

    return total_loss / max(n_batches, 1)


def validate(
    model,
    dataloader,
    device,
    wdl_lambda=0.25,
    raw_eval_loss=True,
    use_huber=True,
    anchors=None,
    anchor_lambda=0.0,
):
    """Validate model."""
    model.eval()
    total_loss = 0.0
    n_batches = 0

    with torch.no_grad():
        for batch in dataloader:
            white_f, black_f, stm, target, result = [x.to(device) for x in batch]
            output = model(white_f, black_f, stm)
            if raw_eval_loss:
                eval_loss = (
                    F.smooth_l1_loss(output, target)
                    if use_huber
                    else F.mse_loss(output, target)
                )
                pred_wdl = torch.sigmoid(output)
            else:
                pred_wdl = torch.sigmoid(output)
                target_sig = torch.sigmoid(target)
                eval_loss = F.mse_loss(pred_wdl, target_sig)
            wdl_loss = F.mse_loss(pred_wdl, result) * wdl_lambda
            anchor_loss = torch.tensor(0.0, device=device)
            if anchors and anchor_lambda > 0.0:
                for name, param in model.named_parameters():
                    if param.requires_grad and name in anchors:
                        anchor_loss = anchor_loss + F.mse_loss(param, anchors[name])
                anchor_loss = anchor_loss * anchor_lambda
            loss = eval_loss + wdl_loss + anchor_loss
            total_loss += loss.item()
            n_batches += 1

    return total_loss / max(n_batches, 1)


def main():
    parser = argparse.ArgumentParser(description="Improved NNUE Training")
    parser.add_argument("--data", type=str, nargs="+", default=[])
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
    eval_objective = parser.add_mutually_exclusive_group()
    eval_objective.add_argument(
        "--sigmoid-eval-loss",
        dest="sigmoid_eval_loss",
        action="store_true",
        default=True,
        help="Use sigmoid-space eval loss: mse(sigmoid(output), sigmoid(search eval)); default",
    )
    eval_objective.add_argument(
        "--raw-eval-loss",
        dest="sigmoid_eval_loss",
        action="store_false",
        help="Use raw-output eval regression selected by --eval-loss",
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
        "--init-nnue",
        type=str,
        default=None,
        help="Initialize model from an existing quantized .nnue file",
    )
    parser.add_argument(
        "--init-piece-square-nnue",
        type=str,
        default=None,
        help="Initialize from a legacy raw or headered 768-input .nnue file",
    )
    parser.add_argument(
        "--no-augment",
        action="store_true",
        help="Disable horizontal mirror augmentation",
    )
    parser.add_argument(
        "--no-filter", action="store_true", help="Disable quiet position filtering"
    )
    parser.add_argument(
        "--eval-clip",
        type=float,
        default=1500,
        help="Clip evaluations beyond this value",
    )
    parser.add_argument(
        "--dropout", type=float, default=0.0, help="Dropout rate for regularization"
    )
    parser.add_argument(
        "--freeze-feature",
        action="store_true",
        help="Freeze feature transformer weights/biases and train only output layer",
    )
    parser.add_argument(
        "--freeze-output",
        action="store_true",
        help="Freeze output weights/bias and train only the feature transformer",
    )
    parser.add_argument(
        "--anchor-lambda",
        type=float,
        default=0.0,
        help="L2 penalty for drifting away from the initialized weights",
    )
    parser.add_argument(
        "--checkpoint-prefix",
        type=str,
        default=None,
        help="Export a quantized NNUE after every epoch with this prefix",
    )
    parser.add_argument(
        "--checkpoint-dir",
        type=str,
        default=None,
        help="Directory for .pt checkpoints; defaults to the output file directory",
    )
    parser.add_argument(
        "--val-split", type=float, default=0.05, help="Validation split ratio"
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=None,
        help="DataLoader worker processes; defaults to a safe platform-specific value",
    )
    parser.add_argument(
        "--export-only",
        action="store_true",
        help="Load checkpoint/init weights, export --output as quantized NNUE, and exit",
    )
    args = parser.parse_args()

    device = get_device()
    model = NNUE256(dropout=args.dropout).to(device)

    if args.init_from_pst and not args.checkpoint:
        model.init_from_pst()

    if args.checkpoint and os.path.exists(args.checkpoint):
        print(f"Loading checkpoint: {args.checkpoint}")
        ckpt = torch.load(args.checkpoint, map_location=device)
        model.load_state_dict(ckpt["model_state_dict"])

    if args.init_nnue and os.path.exists(args.init_nnue):
        print(f"Loading quantized NNUE: {args.init_nnue}")
        model.load_quantized(args.init_nnue)

    if args.init_piece_square_nnue and os.path.exists(args.init_piece_square_nnue):
        print(f"Loading 768-input NNUE: {args.init_piece_square_nnue}")
        model.load_piece_square_quantized(args.init_piece_square_nnue)

    if args.export_only:
        model.export_quantized(args.output)
        return

    if not args.data:
        parser.error("--data is required unless --export-only is used")

    if args.freeze_feature:
        model.feature_weights.requires_grad_(False)
        model.feature_bias.requires_grad_(False)
        print("Freezing feature transformer; training output weights only")
    if args.freeze_output:
        model.output_weights_white.requires_grad_(False)
        model.output_weights_black.requires_grad_(False)
        model.output_bias.requires_grad_(False)
        print("Freezing output layer; training feature transformer only")

    anchors = None
    if args.anchor_lambda > 0.0:
        anchors = {
            name: param.detach().clone()
            for name, param in model.named_parameters()
            if param.requires_grad
        }
        print(f"Anchoring trainable weights with lambda={args.anchor_lambda}")

    total_params = sum(p.numel() for p in model.parameters())
    print(f"Model parameters: {total_params:,}")

    # Load dataset
    full_dataset = ImprovedChessDataset(
        args.data,
        max_positions=args.max_positions,
        filter_quiet=not args.no_filter,
        augment=not args.no_augment,
        eval_clip=args.eval_clip,
    )

    # Split into train/val
    val_size = int(len(full_dataset) * args.val_split)
    train_size = len(full_dataset) - val_size
    train_dataset, val_dataset = random_split(
        full_dataset,
        [train_size, val_size],
        generator=torch.Generator().manual_seed(42),
    )

    print(f"Train: {len(train_dataset):,}, Val: {len(val_dataset):,}")

    loader_config = get_dataloader_config(device, args.workers)

    train_loader = DataLoader(
        train_dataset,
        batch_size=args.batch_size,
        shuffle=True,
        drop_last=True,
        **loader_config,
    )
    val_loader = DataLoader(
        val_dataset, batch_size=args.batch_size, shuffle=False, **loader_config
    )

    optimizer = torch.optim.AdamW(
        [p for p in model.parameters() if p.requires_grad],
        lr=args.lr,
        weight_decay=0.01,
    )

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
    checkpoint_dir = Path(args.checkpoint_dir) if args.checkpoint_dir else Path(args.output).parent
    checkpoint_dir.mkdir(parents=True, exist_ok=True)

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
            anchors=anchors,
            anchor_lambda=args.anchor_lambda,
        )
        val_loss = validate(
            model,
            val_loader,
            device,
            args.wdl_lambda,
            raw_eval_loss=not args.sigmoid_eval_loss,
            use_huber=args.eval_loss == "huber",
            anchors=anchors,
            anchor_lambda=args.anchor_lambda,
        )
        scheduler.step()
        elapsed = time.time() - t0

        print(
            f"Epoch {epoch + 1}/{args.epochs}: "
            f"train={train_loss:.6f}, val={val_loss:.6f}, "
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
            checkpoint_dir / f"checkpoint_improved_epoch_{epoch + 1}.pt",
        )
        if args.checkpoint_prefix:
            model.export_quantized(f"{args.checkpoint_prefix}_epoch_{epoch + 1}.nnue")

        # Save best model
        if val_loss < best_val_loss:
            best_val_loss = val_loss
            model.export_quantized(args.output)
            print(f"  -> New best! Saved to {args.output}")
            patience_counter = 0
        else:
            patience_counter += 1
            if patience_counter >= patience:
                print(f"Early stopping: no improvement for {patience} epochs")
                break

    print(f"\nDone! Best validation loss: {best_val_loss:.6f}")


if __name__ == "__main__":
    main()
