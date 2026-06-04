# Pure NNUE Tactical Feature Investigation

## Current NNUE

The current NNUE input is only piece-square occupancy:

- `INPUT_SIZE = 768`
- index layout: `color * 384 + piece * 64 + oriented_square`
- hidden size currently set to `256`
- file size: `394754` bytes

This is cheap because the search incrementally updates the accumulator for
captures, moves, promotions, castling, and en passant. It does not encode
attacks, pinned pieces, hanging pieces, king pressure, or mobility directly.

## Existing Engine Support

The HCE already computes the tactical information that is missing from NNUE:

- `Board::compute_attack_context()` in `src/board/eval_terms/helpers.rs`
- `Board::eval_hanging_with_context()` in `src/board/eval_terms/hanging.rs`
- `Board::pinned_pieces()` in `src/board/movegen/kings.rs`
- king zone masks and attack tables in `src/board/masks.rs`
- mobility and threat terms under `src/board/eval_terms/`

So the problem is not inventing the features. The problem is exposing them to
the NNUE without destroying search speed or accumulator correctness.

## Feature Options

### Option A: Compact Dynamic Tactical Features

Keep the existing 768 piece-square inputs and append sparse tactical features:

- hanging piece: `color * piece * square`
- attacked by pawn: `color * piece * square`
- pinned piece: `color * piece * square`
- side/color in check
- king ring pressure buckets by attacking piece type
- mobility buckets by color/piece type

Approximate input size with a first compact layout:

- base piece-square: `768`
- hanging non-king piece-square: `2 * 5 * 64 = 640`
- pawn-attacked non-king piece-square: `640`
- pinned non-king piece-square: `640`
- in-check flags: `4`
- king pressure buckets: about `96`
- total: about `2788`

At hidden size 256 this is about `1.36 MiB`.

This is the best near-term route. It keeps the model small and lets us train
from the existing `768-256-1` net by copying the first 768 feature rows and
initializing the new tactical rows near zero.

### Option B: Full King-Conditioned HalfKP

Use king-square-conditioned piece features, closer to classic NNUE:

- side/perspective king square
- piece/color/square relative to that king

This is materially stronger than plain 768 occupancy, but much larger:

- approximate input size: `64 * 2 * 10 * 64 = 81920`
- hidden size 256 file size: about `40 MiB`

This is likely stronger long-term, but it requires a new training pipeline,
new binary format expectations, and careful incremental king-move handling.

### Option C: Second Dynamic Accumulator

Keep the existing incremental accumulator for piece-square inputs, then at
evaluation time clone it and add dynamic tactical features computed from the
current board.

This is the least invasive runtime design because tactical features do not
need incremental update code. It costs roughly:

`active_dynamic_features * HIDDEN_SIZE`

extra adds per eval. With around 10-30 active tactical features and hidden 256,
that is probably acceptable for experiments. If it is too slow, the same
features can later be cached or incrementally maintained.

## Recommended Experiment

Implement Option A using Option C mechanically:

1. Increase `INPUT_SIZE` from `768` to about `2788`.
2. Keep old piece-square feature rows exactly compatible.
3. Add `Board::compute_nnue_dynamic_features()` returning sparse feature
   indices for both perspectives.
4. In `evaluate_simple()`, clone the current accumulator, add dynamic feature
   rows, then evaluate.
5. Update `scripts/train_nnue_improved.py` and `scripts/train_nnue_ranking.py`
   to emit the same feature indices from FEN.
6. Initialize from `runs/nnue_256_29M.nnue` by copying the first 768 rows and
   zeroing or tiny-random-initializing the new rows.
7. Train first on HCE/hybrid root ranking, then evaluate WAC every epoch.

## Expected Impact

The earlier experiments show that the plain scalar occupancy NNUE saturates
around the low/mid 150s on WAC. HCE-only reaches about 176 under the current
binary, and HCE-blended NNUE was much higher in prior runs. The missing signal
is tactical, not just value calibration.

The compact tactical feature net is the smallest change that can plausibly
move pure NNUE toward the HCE/blended range without using HCE at runtime.
