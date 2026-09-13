# NNUE inference and export contract

The Rust engine uses 256 hidden units. Raw files contain 768 piece-square
inputs. Headered files start with `RNQNNUE\0`, followed by little-endian
`u32` version, input count, and hidden count fields; the current version is
1. The current tactical input layout contains 2818 features.

The payload contains little-endian signed 16-bit values in this order:

1. Feature weights, grouped by input feature, then hidden unit.
2. Hidden biases.
3. Output weights for the side-to-move accumulator.
4. Output weights for the opponent accumulator.
5. One output bias.

In memory, `NnueAccumulator.white` and `.black` contain 32-bit sums. Weights
remain 16-bit on disk and in memory. Intermediate sums must not saturate:
clipping before all features have been added makes the result depend on
feature order and prevents removals from reversing earlier additions.
Only the final SCReLU activation clamps values to `[0, QA]`.

The output blocks retain their historical `output_weights_white` and
`output_weights_black` names in checkpoints and Rust structs. Their roles
are always **us** and **them**. When Black moves, only the accumulators
swap; swapping the weights as well cancels the side-to-move selection.

Training outputs use units of 400 centipawns. Feature weights and hidden
biases use `QA = 255`, output weights use `QB = 64`, and the output bias is
quantized as `round(float_bias * QA * QB)`. Runtime evaluation computes:

```
(dot(clipped_us², us_weights) + dot(clipped_them², them_weights)
 + quantized_bias * QA) * 400 / (QA² * QB)
```

Dividing the bias by 400 during export loses its contribution. Loading a
quantized file into a trainer divides its stored bias by `QA * QB`, which
preserves the file's runtime meaning. To recover the intended bias from
an older training run that used the incorrect export scale, re-export its
original floating-point checkpoint with the corrected trainer.

The side-to-move fix changes Black-to-move predictions from existing
networks without changing their file layout or weights. Historical
training and playing-strength results should be measured again after
this correction.

Run the training contract tests with:

```sh
python3 -m unittest discover -s tests -p test_nnue_training.py
```

These require the trainers' PyTorch and NumPy dependencies and check both
turns, exported scores, quantized reloads, label orientation, and the
packaged trainers' hidden size. Rust's network tests verify inference uses
the same accumulator roles.
