NNUE Training Package (256 hidden, sigmoid-space)
======================================================

Setup:
  pip install torch python-chess

Training (NVIDIA GPU):
  python3 train_nnue_improved.py \
    --data sf18_d14_full_extracted.txt \
    --output nnue_256_sigmoid.nnue \
    --epochs 30 \
    --batch-size 16384 \
    --lr 0.000875 \
    --wdl-lambda 0.0 \
    --eval-clip 1500 \
    --sigmoid-eval-loss \
    --max-positions 75000000 \
    --val-split 0.05 \
    --no-filter

Architecture and training settings:
  - HIDDEN_SIZE: 256, matching src/board/nnue/network.rs
  - Loss: raw Huber -> sigmoid-space MSE 
  - Data: 25M -> 50-75M positions
  - Batch: 8192 -> 16384
  - LR: 0.001 -> 0.000875
  - eval_clip: 800 -> 1500 (include more tactical positions)
  - Removed quiet position filtering

Data file needed:
  sf18_d14_full_extracted.txt (8.8GB, 144M positions)
  OR sf18_d14_25M.txt (1.5GB, 25M positions - smaller subset)

After training, copy .nnue to engine:
  cp nnue_256_sigmoid.nnue src/board/nnue/nets/default.nnue

Scores and game results in the training file are from White's perspective.
The trainer converts both to the side-to-move perspective. Output weights
have fixed us/them roles, and the exported output bias uses QA * QB scaling.
Earlier 512-hidden exports and checkpoints do not match the engine's
256-hidden architecture; train with the compatible configuration above.
