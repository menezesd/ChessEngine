NNUE Training Package (v2 - 512 hidden, sigmoid-space)
======================================================

Setup:
  pip install torch python-chess

Training (NVIDIA GPU):
  python3 train_nnue_improved.py \
    --data sf18_d14_full_extracted.txt \
    --output nnue_512_sigmoid.nnue \
    --epochs 30 \
    --batch-size 16384 \
    --lr 0.000875 \
    --wdl-lambda 0.0 \
    --eval-clip 1500 \
    --sigmoid-eval-loss \
    --max-positions 75000000 \
    --val-split 0.05 \
    --no-filter

Changes from v1:
  - HIDDEN_SIZE: 256 -> 512 (2x capacity)
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
  cp nnue_512_sigmoid.nnue src/board/nnue/nets/default.nnue
