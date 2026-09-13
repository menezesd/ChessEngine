//! Vectorizable NNUE accumulator operations.
//!
//! Accumulate i16 weights in i32 lanes: saturating an intermediate sum loses
//! information and makes both feature order and incremental updates matter.
//! Even the sum of every supported input row fits in i32. The activation is
//! clamped only at inference, and the output reduction uses i64.

use super::{HIDDEN_SIZE, QA};

/// Add quantized weights without clipping intermediate activations.
#[inline]
pub fn add_weights(acc: &mut [i32; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    for (value, weight) in acc.iter_mut().zip(weights) {
        *value += i32::from(*weight);
    }
}

/// Remove quantized weights, exactly reversing an earlier addition.
#[inline]
pub fn sub_weights(acc: &mut [i32; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    for (value, weight) in acc.iter_mut().zip(weights) {
        *value -= i32::from(*weight);
    }
}

/// Compute the `SCReLU` activation and output dot product.
#[inline]
#[must_use]
pub fn screlu_dot(acc: &[i32; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) -> i64 {
    acc.iter()
        .zip(weights)
        .map(|(&value, &weight)| {
            let activated = value.clamp(0, QA);
            // QA^2 * any i16 weight fits in i32; widen before reducing.
            i64::from(activated * activated * i32::from(weight))
        })
        .sum()
}

#[cfg(test)]
mod tests;
