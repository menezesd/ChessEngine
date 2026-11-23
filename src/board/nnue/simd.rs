//! Vectorizable NNUE accumulator operations.
//!
//! Accumulate i16 weights in i32 lanes: saturating an intermediate sum loses
//! information and makes both feature order and incremental updates matter.
//! Even the sum of every supported input row fits in i32. The activation is
//! clamped only at inference. The wide output reduction sums in i64.

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

/// Copy the parent activations while applying an ordinary move's deltas.
#[inline]
pub(super) fn move_weights(
    parent: &[i32; HIDDEN_SIZE],
    child: &mut [i32; HIDDEN_SIZE],
    added: &[i16; HIDDEN_SIZE],
    removed: &[i16; HIDDEN_SIZE],
    captured: Option<&[i16; HIDDEN_SIZE]>,
) {
    if let Some(captured) = captured {
        for (index, lane) in child.iter_mut().enumerate() {
            *lane = parent[index] + i32::from(added[index])
                - i32::from(removed[index])
                - i32::from(captured[index]);
        }
    } else {
        for (index, lane) in child.iter_mut().enumerate() {
            *lane = parent[index] + i32::from(added[index]) - i32::from(removed[index]);
        }
    }
}

/// Compute the `SCReLU` activation and output dot product.
#[inline]
#[must_use]
pub fn screlu_dot(acc: &[i32; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) -> i64 {
    acc.iter()
        .zip(weights)
        // Widen before reducing arbitrary weights.
        .map(|(&value, &weight)| i64::from(screlu_product(value, weight)))
        .sum()
}

/// The loaded weights bound all partial sums to i32.
#[inline]
pub(super) fn screlu_dot_i32(acc: &[i32; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) -> i32 {
    acc.iter()
        .zip(weights)
        .map(|(&value, &weight)| screlu_product(value, weight))
        .sum()
}

#[inline]
fn screlu_product(value: i32, weight: i16) -> i32 {
    let activated = value.clamp(0, QA);
    // QA squared times any i16 weight fits in i32.
    activated * activated * i32::from(weight)
}

#[cfg(test)]
mod tests;
