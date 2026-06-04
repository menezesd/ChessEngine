#!/usr/bin/env python3
"""
NNUE Training Script - 256 Hidden Architecture
Matches the Rust implementation: (768 -> 256) x 2 perspectives -> 1
"""

import argparse
import os
import struct
import time
from pathlib import Path
from typing import List, Tuple

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.utils.data import Dataset, DataLoader

# Constants matching Rust implementation
INPUT_SIZE = 768   # 64 squares * 6 pieces * 2 colors
HIDDEN_SIZE = 256  # Must match Rust network.rs
SCALE = 400
QA = 255  # Weight quantization factor
QB = 64   # Output weight quantization factor

PIECE_MAP = {'P': 0, 'N': 1, 'B': 2, 'R': 3, 'Q': 4, 'K': 5,
             'p': 0, 'n': 1, 'b': 2, 'r': 3, 'q': 4, 'k': 5}

# HCE Piece-Square Tables from pst.rs (tuned on ~5M positions)
MATERIAL_MG = [49, 278, 315, 394, 1031, 20000]
MATERIAL_EG = [82, 254, 253, 443, 911, 20000]

PST_MG = [
    # Pawn
    [0,0,0,0,0,0,0,0,-35,-1,-20,-23,-15,24,38,-22,-26,-4,-4,-10,3,3,33,-12,
     -27,-2,-5,12,17,6,10,-25,-14,13,6,21,23,12,17,-23,-6,7,26,31,64,55,25,-20,
     97,132,60,94,67,124,34,-11,0,0,0,0,0,0,0,0],
    # Knight
    [-104,-21,-57,-33,-17,-28,-19,-23,-29,-52,-12,-3,-1,18,-14,-19,-23,-9,12,10,19,17,25,-16,
     -13,4,16,13,28,19,21,-8,-9,17,19,52,37,68,18,22,-46,59,37,64,83,127,72,44,
     -72,-41,71,36,23,61,7,-17,-164,-88,-34,-48,60,-96,-15,-106],
    # Bishop
    [-33,-3,-14,-21,-13,-12,-39,-21,4,15,16,0,7,21,33,1,0,15,15,15,14,27,18,10,
     -6,13,13,26,34,12,10,4,-4,5,19,49,37,37,7,-2,-16,37,43,40,35,49,37,-2,
     -26,16,-18,-13,30,58,18,-46,-29,4,-81,-37,-25,-42,7,-8],
    # Rook
    [-19,-13,1,17,16,7,-37,-26,-44,-16,-20,-9,-1,11,-6,-70,-45,-25,-16,-17,3,0,-5,-33,
     -36,-26,-12,-1,9,-7,6,-23,-24,-11,7,26,24,35,-8,-20,-5,19,26,36,17,45,60,16,
     27,32,57,61,79,66,26,44,32,42,32,50,62,9,31,43],
    # Queen
    [-1,-18,-9,10,-15,-25,-31,-49,-35,-8,11,2,8,15,-3,1,-14,2,-11,-2,-5,2,14,5,
     -9,-26,-9,-10,-2,-4,3,-3,-27,-27,-16,-16,-1,17,-2,1,-13,-17,7,8,29,55,46,56,
     -24,-39,-5,1,-16,56,28,53,-28,0,29,12,58,44,43,45],
    # King
    [-15,36,12,-53,8,-28,34,14,1,7,-8,-63,-43,-16,9,8,-14,-14,-22,-46,-44,-30,-15,-27,
     -48,-1,-27,-39,-46,-44,-33,-50,-17,-20,-12,-27,-30,-25,-14,-36,-9,24,2,-16,-20,6,22,-22,
     29,-1,-20,-7,-8,-4,-38,-29,-64,23,16,-15,-55,-34,2,13]
]

PST_EG = [
    # Pawn
    [0,0,0,0,0,0,0,0,13,8,8,10,13,0,2,-7,4,7,-6,1,0,-5,-1,-8,
     13,9,-3,-7,-7,-8,3,-1,32,24,13,5,-2,4,17,17,93,99,84,66,55,52,81,83,
     175,170,155,132,144,130,162,184,0,0,0,0,0,0,0,0],
    # Knight
    [-29,-50,-23,-15,-22,-18,-49,-63,-42,-20,-10,-5,-2,-20,-23,-44,-23,-3,-1,15,10,-3,-20,-22,
     -18,-6,16,25,16,17,4,-18,-17,3,22,22,22,11,8,-18,-24,-20,10,9,-1,-9,-19,-41,
     -25,-8,-25,-2,-9,-25,-24,-51,-57,-38,-13,-28,-31,-27,-62,-98],
    # Bishop
    [-23,-9,-23,-5,-9,-16,-5,-17,-14,-18,-7,-1,4,-9,-15,-27,-12,-3,8,10,13,3,-7,-15,
     -6,3,13,19,7,10,-3,-9,-3,9,12,9,14,10,3,2,2,-8,0,-1,-2,6,0,4,
     -8,-4,7,-12,-3,-13,-4,-14,-14,-21,-11,-8,-7,-9,-17,-24],
    # Rook
    [-9,2,3,-1,-5,-13,4,-20,-6,-6,0,2,-9,-9,-11,-3,-4,0,-5,-1,-7,-12,-8,-16,
     3,5,8,4,-5,-6,-8,-11,4,3,13,1,2,1,-1,2,7,7,7,5,4,-3,-5,-3,
     11,13,13,11,-3,3,8,3,13,10,18,15,12,12,8,5],
    # Queen
    [-33,-28,-22,-43,-5,-32,-20,-41,-22,-23,-30,-16,-16,-23,-36,-32,-16,-27,15,6,9,17,10,5,
     -18,28,19,46,31,34,39,23,3,22,24,45,56,40,56,36,-20,6,9,48,46,35,19,9,
     -17,20,32,41,57,25,30,0,-9,22,22,27,27,19,10,20],
    # King
    [-52,-34,-21,-11,-28,-14,-24,-43,-27,-11,4,13,14,4,-5,-17,-19,-3,11,21,23,16,7,-9,
     -18,-4,21,24,27,23,9,-11,-8,22,24,27,26,33,26,3,10,17,23,15,20,45,44,13,
     -12,17,14,17,17,38,23,11,-73,-35,-18,-18,-11,15,4,-17]
]


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


def feature_index(piece_type: int, piece_color: int, square: int, perspective: int) -> int:
    """Compute feature index for NNUE."""
    if perspective == 1:  # Black's perspective - flip board
        oriented_sq = square ^ 56
        oriented_color = 1 - piece_color
    else:  # White's perspective
        oriented_sq = square
        oriented_color = piece_color
    return oriented_color * 384 + piece_type * 64 + oriented_sq


def parse_fen_features(fen: str) -> Tuple[List[int], List[int], bool]:
    """Parse FEN and return feature indices for both perspectives."""
    parts = fen.split()
    board = parts[0]
    side_to_move = parts[1] if len(parts) > 1 else 'w'
    white_to_move = (side_to_move == 'w')

    white_features = []
    black_features = []
    square = 56

    for char in board:
        if char == '/':
            square -= 16
        elif char.isdigit():
            square += int(char)
        else:
            piece_type = PIECE_MAP[char]
            piece_color = 0 if char.isupper() else 1
            white_features.append(feature_index(piece_type, piece_color, square, 0))
            black_features.append(feature_index(piece_type, piece_color, square, 1))
            square += 1

    return white_features, black_features, white_to_move


class NNUE256(nn.Module):
    """
    NNUE Network matching Rust implementation.
    Architecture: (768 -> 256) x 2 perspectives -> 1
    """

    def __init__(self):
        super().__init__()
        # Feature transformer
        self.feature_weights = nn.Parameter(torch.zeros(INPUT_SIZE, HIDDEN_SIZE))
        self.feature_bias = nn.Parameter(torch.zeros(HIDDEN_SIZE))
        # Output layer (separate for white/black perspective)
        self.output_weights_white = nn.Parameter(torch.zeros(HIDDEN_SIZE))
        self.output_weights_black = nn.Parameter(torch.zeros(HIDDEN_SIZE))
        self.output_bias = nn.Parameter(torch.zeros(1))
        self._init_weights()

    def _init_weights(self):
        nn.init.kaiming_normal_(self.feature_weights, mode='fan_out', nonlinearity='relu')
        self.feature_weights.data *= 0.3
        nn.init.zeros_(self.feature_bias)
        nn.init.normal_(self.output_weights_white, std=0.1)
        nn.init.normal_(self.output_weights_black, std=0.1)
        nn.init.zeros_(self.output_bias)

    def init_from_pst(self):
        """Initialize feature weights from HCE piece-square tables."""
        print("Initializing NNUE from HCE piece-square tables...")

        # Average MG and EG for simplicity (could use game phase but we don't have it per-position)
        with torch.no_grad():
            # Zero out feature weights first
            self.feature_weights.data.zero_()

            # For each feature (piece_color, piece_type, square), set initial weights
            # Feature index: color * 384 + piece * 64 + square
            for piece_color in range(2):  # 0=white pieces, 1=black pieces
                for piece_type in range(6):  # P, N, B, R, Q, K
                    for square in range(64):
                        feature_idx = piece_color * 384 + piece_type * 64 + square

                        # Get PST value (from white's perspective)
                        # For black pieces, flip the square vertically
                        if piece_color == 0:  # White piece
                            pst_sq = square
                            sign = 1
                        else:  # Black piece
                            pst_sq = square ^ 56  # Flip vertically
                            sign = -1  # Black pieces are negative for white

                        # Average of MG and EG PST values + material
                        pst_mg = PST_MG[piece_type][pst_sq]
                        pst_eg = PST_EG[piece_type][pst_sq]
                        material_mg = MATERIAL_MG[piece_type]
                        material_eg = MATERIAL_EG[piece_type]

                        # Combined value (PST + material), scaled for NNUE
                        # Use 0.7 MG + 0.3 EG blend
                        pst_value = 0.7 * pst_mg + 0.3 * pst_eg
                        material_value = 0.7 * material_mg + 0.3 * material_eg

                        # For non-king pieces, include material; for king, just PST
                        if piece_type < 5:
                            total_value = (pst_value + material_value) * sign
                        else:
                            total_value = pst_value * sign

                        # Scale to NNUE range (divide by SCALE and distribute across hidden units)
                        # The value will be summed across hidden units, so divide by sqrt(HIDDEN_SIZE)
                        scaled_value = total_value / SCALE / (HIDDEN_SIZE ** 0.5)

                        # Set the first few hidden units to capture PST information
                        # Use different patterns for different piece types
                        for h in range(HIDDEN_SIZE):
                            # Distribute PST info across hidden units with some variation
                            self.feature_weights.data[feature_idx, h] = scaled_value * (0.8 + 0.4 * ((h + piece_type) % 5) / 5)

            # Initialize output weights to sum hidden units evenly
            self.output_weights_white.data.fill_(1.0 / (HIDDEN_SIZE ** 0.5))
            self.output_weights_black.data.fill_(1.0 / (HIDDEN_SIZE ** 0.5))

        print("  PST initialization complete")

    def forward(self, white_features: torch.Tensor, black_features: torch.Tensor,
                white_to_move: torch.Tensor) -> torch.Tensor:
        """Forward pass."""
        # Compute accumulators
        white_acc = F.linear(white_features, self.feature_weights.t(), self.feature_bias)
        black_acc = F.linear(black_features, self.feature_weights.t(), self.feature_bias)

        # SCReLU activation (clamp to [0, 1], then square)
        white_acc = torch.clamp(white_acc, 0, 1) ** 2
        black_acc = torch.clamp(black_acc, 0, 1) ** 2

        # Select us/them based on side to move
        white_to_move_exp = white_to_move.unsqueeze(1)
        us_acc = torch.where(white_to_move_exp, white_acc, black_acc)
        them_acc = torch.where(white_to_move_exp, black_acc, white_acc)

        # Output computation
        us_weights = torch.where(white_to_move_exp,
                                  self.output_weights_white.unsqueeze(0),
                                  self.output_weights_black.unsqueeze(0))
        them_weights = torch.where(white_to_move_exp,
                                    self.output_weights_black.unsqueeze(0),
                                    self.output_weights_white.unsqueeze(0))

        output = (us_acc * us_weights).sum(dim=1) + \
                 (them_acc * them_weights).sum(dim=1) + \
                 self.output_bias.squeeze()

        return output.unsqueeze(1)

    def export_quantized(self, path: str):
        """Export to quantized binary format for Rust."""
        with open(path, 'wb') as f:
            # Feature weights: INPUT_SIZE columns of HIDDEN_SIZE i16 values
            weights = (self.feature_weights.detach().cpu().numpy() * QA).round().clip(-32768, 32767).astype('<i2')
            for i in range(INPUT_SIZE):
                for j in range(HIDDEN_SIZE):
                    f.write(struct.pack('<h', int(weights[i, j])))

            # Feature bias with 64-byte alignment
            bias = (self.feature_bias.detach().cpu().numpy() * QA).round().clip(-32768, 32767).astype('<i2')
            for j in range(HIDDEN_SIZE):
                f.write(struct.pack('<h', int(bias[j])))

            # Output weights white
            out_w = (self.output_weights_white.detach().cpu().numpy() * QB).round().clip(-32768, 32767)
            for j in range(HIDDEN_SIZE):
                f.write(struct.pack('<h', int(out_w[j])))

            # Output weights black
            out_b = (self.output_weights_black.detach().cpu().numpy() * QB).round().clip(-32768, 32767)
            for j in range(HIDDEN_SIZE):
                f.write(struct.pack('<h', int(out_b[j])))

            # Output bias
            out_bias = (self.output_bias.detach().cpu().numpy() * QA * QB / SCALE).round().clip(-32768, 32767)
            f.write(struct.pack('<h', int(out_bias[0])))

        print(f"Exported network to {path} ({os.path.getsize(path)} bytes)")


class ChessDataset(Dataset):
    """Dataset for chess training data. Format: FEN | eval | result"""

    def __init__(self, data_files: List[str], max_positions: int = None):
        self.positions = []
        count = 0
        for data_file in data_files:
            print(f"Loading {data_file}...")
            with open(data_file, 'r') as f:
                for line in f:
                    if max_positions and count >= max_positions:
                        break
                    line = line.strip()
                    if not line:
                        continue
                    parts = line.split('|')
                    if len(parts) < 2:
                        continue
                    fen = parts[0].strip()
                    try:
                        eval_score = float(parts[1].strip())
                    except ValueError:
                        continue

                    # Clamp extreme evals
                    eval_score = max(-2000, min(2000, eval_score))

                    game_result = 0.5
                    if len(parts) >= 3:
                        r = parts[2].strip()
                        if r in ('1.0', '1'):
                            game_result = 1.0
                        elif r in ('0.0', '0'):
                            game_result = 0.0

                    try:
                        white_feat, black_feat, stm = parse_fen_features(fen)
                        # Convert game result to side-to-move perspective
                        if not stm:  # Black to move
                            stm_result = 1.0 - game_result
                        else:
                            stm_result = game_result
                        self.positions.append((white_feat, black_feat, stm, eval_score, stm_result))
                        count += 1
                    except Exception:
                        continue
            if max_positions and count >= max_positions:
                break
        print(f"Loaded {len(self.positions):,} positions")

    def __len__(self):
        return len(self.positions)

    def __getitem__(self, idx):
        white_feat, black_feat, stm, eval_score, result = self.positions[idx]

        white_tensor = torch.zeros(INPUT_SIZE, dtype=torch.float32)
        black_tensor = torch.zeros(INPUT_SIZE, dtype=torch.float32)
        for i in white_feat:
            white_tensor[i] = 1.0
        for i in black_feat:
            black_tensor[i] = 1.0

        return (white_tensor, black_tensor,
                torch.tensor(stm, dtype=torch.bool),
                torch.tensor([eval_score / SCALE], dtype=torch.float32),
                torch.tensor([result], dtype=torch.float32))


def train_epoch(model, dataloader, optimizer, device, wdl_lambda=0.5):
    """Train one epoch."""
    model.train()
    total_loss = 0.0
    n_batches = 0

    for batch in dataloader:
        white_f, black_f, stm, target, result = [x.to(device) for x in batch]

        optimizer.zero_grad()
        output = model(white_f, black_f, stm)

        # Sigmoid MSE loss for eval
        pred_sig = torch.sigmoid(output)
        target_sig = torch.sigmoid(target)
        eval_loss = F.mse_loss(pred_sig, target_sig)

        # WDL loss
        wdl_loss = F.mse_loss(pred_sig, result) * wdl_lambda

        loss = eval_loss + wdl_loss
        loss.backward()

        # Gradient clipping
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        optimizer.step()

        total_loss += loss.item()
        n_batches += 1

        if n_batches % 500 == 0:
            print(f"  Batch {n_batches}: loss={total_loss/n_batches:.6f}")

    return total_loss / max(n_batches, 1)


def main():
    parser = argparse.ArgumentParser(description='Train NNUE (256 hidden)')
    parser.add_argument('--data', type=str, nargs='+', required=True)
    parser.add_argument('--output', type=str, default='trained_256.nnue')
    parser.add_argument('--epochs', type=int, default=10)
    parser.add_argument('--batch-size', type=int, default=8192)
    parser.add_argument('--lr', type=float, default=0.001)
    parser.add_argument('--wdl-lambda', type=float, default=0.5)
    parser.add_argument('--max-positions', type=int, default=None)
    parser.add_argument('--checkpoint', type=str, default=None)
    parser.add_argument('--init-from-pst', action='store_true',
                        help='Initialize NNUE weights from HCE piece-square tables')
    args = parser.parse_args()

    device = get_device()
    model = NNUE256().to(device)

    # Initialize from PST if requested (before loading checkpoint)
    if args.init_from_pst and not args.checkpoint:
        model.init_from_pst()

    if args.checkpoint and os.path.exists(args.checkpoint):
        print(f"Loading checkpoint: {args.checkpoint}")
        ckpt = torch.load(args.checkpoint, map_location=device)
        model.load_state_dict(ckpt['model_state_dict'])

    total_params = sum(p.numel() for p in model.parameters())
    print(f"Model parameters: {total_params:,}")

    dataset = ChessDataset(args.data, args.max_positions)
    dataloader = DataLoader(dataset, batch_size=args.batch_size, shuffle=True,
                           num_workers=4, pin_memory=True, drop_last=True)

    optimizer = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=0.01)
    scheduler = torch.optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=args.epochs, eta_min=args.lr*0.01)

    print(f"\nTraining: {args.epochs} epochs, batch={args.batch_size}, lr={args.lr}")
    best_loss = float('inf')

    for epoch in range(args.epochs):
        t0 = time.time()
        loss = train_epoch(model, dataloader, optimizer, device, args.wdl_lambda)
        scheduler.step()
        elapsed = time.time() - t0

        print(f"Epoch {epoch+1}/{args.epochs}: loss={loss:.6f}, lr={optimizer.param_groups[0]['lr']:.6f}, time={elapsed:.1f}s")

        # Save checkpoint
        torch.save({'epoch': epoch+1, 'model_state_dict': model.state_dict(), 'loss': loss},
                   f'checkpoint_256_epoch_{epoch+1}.pt')

        if loss < best_loss:
            best_loss = loss
            model.export_quantized(args.output)
            print(f"  -> New best! Saved to {args.output}")

    print(f"\nDone! Best loss: {best_loss:.6f}")


if __name__ == '__main__':
    main()
