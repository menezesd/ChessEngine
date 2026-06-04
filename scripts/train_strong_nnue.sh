#!/bin/bash
# Train a strong NNUE network
# Usage: ./scripts/train_strong_nnue.sh [epochs] [eval_clip]
# Default: 40 epochs, eval_clip=800

EPOCHS=${1:-40}
EVAL_CLIP=${2:-800}
OUTPUT="runs/strong_nnue.nnue"
LOG="runs/strong_nnue.log"

mkdir -p runs

echo "Training NNUE: ${EPOCHS} epochs, eval_clip=${EVAL_CLIP}"
echo "Output: ${OUTPUT}"
echo "Log: ${LOG}"
echo ""

python3 scripts/train_nnue_improved.py \
    --data data/sf18_d14_25M.txt \
    --output "${OUTPUT}" \
    --epochs "${EPOCHS}" \
    --batch-size 8192 \
    --lr 0.001 \
    --wdl-lambda 0.0 \
    --eval-clip "${EVAL_CLIP}" \
    --eval-loss huber \
    --val-split 0.05 \
    2>&1 | tee "${LOG}"

echo ""
echo "Training complete. Network saved to ${OUTPUT}"
echo "Copy to engine: cp ${OUTPUT} src/board/nnue/nets/default.nnue"
