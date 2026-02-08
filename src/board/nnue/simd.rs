//! SIMD-optimized operations for NNUE evaluation.
//!
//! Provides vectorized implementations for:
//! - Accumulator updates (add/subtract i16 vectors)
//! - `SCReLU` activation with dot product
//!
//! Supports:
//! - `x86_64`: `AVX2` (256-bit vectors, 16 i16 at a time)
//! - aarch64: NEON (128-bit vectors, 8 i16 at a time)
//! - Fallback: Scalar operations

use super::network::HIDDEN_SIZE;

/// Weight quantization factor (must match parent module)
const QA: i16 = 255;

// ============================================================================
// Public API - dispatches to platform-specific implementations
// ============================================================================

/// Add weights to accumulator using SIMD when available.
#[inline]
pub fn add_weights(acc: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    #[cfg(target_arch = "aarch64")]
    {
        // NEON is always available on aarch64
        unsafe { add_weights_neon(acc, weights) }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        unsafe { add_weights_avx2(acc, weights) }
    }

    #[cfg(all(target_arch = "x86_64", not(target_feature = "avx2")))]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { add_weights_avx2(acc, weights) }
        } else {
            add_weights_scalar(acc, weights)
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        add_weights_scalar(acc, weights)
    }
}

/// Subtract weights from accumulator using SIMD when available.
#[inline]
pub fn sub_weights(acc: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    #[cfg(target_arch = "aarch64")]
    {
        unsafe { sub_weights_neon(acc, weights) }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        unsafe { sub_weights_avx2(acc, weights) }
    }

    #[cfg(all(target_arch = "x86_64", not(target_feature = "avx2")))]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { sub_weights_avx2(acc, weights) }
        } else {
            sub_weights_scalar(acc, weights)
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        sub_weights_scalar(acc, weights)
    }
}

/// Compute `SCReLU` activation and dot product using SIMD when available.
///
/// Returns sum of: `screlu(acc[i]) * weights[i]` for i in `0..HIDDEN_SIZE`
#[inline]
#[must_use]
pub fn screlu_dot(acc: &[i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) -> i32 {
    #[cfg(target_arch = "aarch64")]
    {
        unsafe { screlu_dot_neon(acc, weights) }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        unsafe { screlu_dot_avx2(acc, weights) }
    }

    #[cfg(all(target_arch = "x86_64", not(target_feature = "avx2")))]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { screlu_dot_avx2(acc, weights) }
        } else {
            screlu_dot_scalar(acc, weights)
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        screlu_dot_scalar(acc, weights)
    }
}

// ============================================================================
// Scalar fallback implementations
// Used on x86_64 without AVX2 and non-SIMD platforms.
// Not used on aarch64 (NEON always available).
// ============================================================================

#[cfg(any(
    not(any(target_arch = "x86_64", target_arch = "aarch64")),
    all(target_arch = "x86_64", not(target_feature = "avx2"))
))]
#[inline]
fn add_weights_scalar(acc: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    for i in 0..HIDDEN_SIZE {
        acc[i] = acc[i].saturating_add(weights[i]);
    }
}

#[cfg(any(
    not(any(target_arch = "x86_64", target_arch = "aarch64")),
    all(target_arch = "x86_64", not(target_feature = "avx2"))
))]
#[inline]
fn sub_weights_scalar(acc: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    for i in 0..HIDDEN_SIZE {
        acc[i] = acc[i].saturating_sub(weights[i]);
    }
}

/// Scalar fallback for `screlu_dot`.
#[cfg(any(
    test,
    not(any(target_arch = "x86_64", target_arch = "aarch64")),
    all(target_arch = "x86_64", not(target_feature = "avx2"))
))]
#[inline]
fn screlu_dot_scalar(acc: &[i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) -> i32 {
    let mut sum = 0i32;
    for i in 0..HIDDEN_SIZE {
        let clamped = i32::from(acc[i]).clamp(0, i32::from(QA));
        let activated = clamped * clamped;
        sum += activated * i32::from(weights[i]);
    }
    sum
}

// ============================================================================
// NEON implementations (aarch64 - Apple Silicon, ARM servers)
// ============================================================================

#[cfg(target_arch = "aarch64")]
unsafe fn add_weights_neon(acc: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    use std::arch::aarch64::{vld1q_s16, vqaddq_s16, vst1q_s16};

    let acc_ptr = acc.as_mut_ptr();
    let weights_ptr = weights.as_ptr();

    // Process 8 i16 values at a time (128 bits)
    for i in (0..HIDDEN_SIZE).step_by(8) {
        let a = vld1q_s16(acc_ptr.add(i));
        let w = vld1q_s16(weights_ptr.add(i));
        let sum = vqaddq_s16(a, w); // Saturating add
        vst1q_s16(acc_ptr.add(i), sum);
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn sub_weights_neon(acc: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    use std::arch::aarch64::{vld1q_s16, vqsubq_s16, vst1q_s16};

    let acc_ptr = acc.as_mut_ptr();
    let weights_ptr = weights.as_ptr();

    for i in (0..HIDDEN_SIZE).step_by(8) {
        let a = vld1q_s16(acc_ptr.add(i));
        let w = vld1q_s16(weights_ptr.add(i));
        let diff = vqsubq_s16(a, w); // Saturating sub
        vst1q_s16(acc_ptr.add(i), diff);
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn screlu_dot_neon(acc: &[i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) -> i32 {
    use std::arch::aarch64::{
        vaddq_s64, vdupq_n_s16, vdupq_n_s64, vget_high_s16, vget_high_s32, vget_low_s16,
        vget_low_s32, vgetq_lane_s64, vld1q_s16, vmaxq_s16, vminq_s16, vmovl_s16, vmovl_s32,
        vmulq_s32,
    };

    let acc_ptr = acc.as_ptr();
    let weights_ptr = weights.as_ptr();

    let zero = vdupq_n_s16(0);
    let qa = vdupq_n_s16(QA);

    // Accumulate in 4 x i64 to avoid overflow
    let mut sum0 = vdupq_n_s64(0);
    let mut sum1 = vdupq_n_s64(0);

    // Process 8 i16 values at a time
    for i in (0..HIDDEN_SIZE).step_by(8) {
        let a = vld1q_s16(acc_ptr.add(i));
        let w = vld1q_s16(weights_ptr.add(i));

        // Clamp to [0, QA]
        let clamped = vminq_s16(vmaxq_s16(a, zero), qa);

        // Split into low and high halves, extend to i32
        let clamped_lo = vmovl_s16(vget_low_s16(clamped)); // 4 x i32
        let clamped_hi = vmovl_s16(vget_high_s16(clamped)); // 4 x i32

        // Square
        let sq_lo = vmulq_s32(clamped_lo, clamped_lo);
        let sq_hi = vmulq_s32(clamped_hi, clamped_hi);

        // Extend weights to i32
        let w_lo = vmovl_s16(vget_low_s16(w));
        let w_hi = vmovl_s16(vget_high_s16(w));

        // Multiply: sq * w
        let prod_lo = vmulq_s32(sq_lo, w_lo);
        let prod_hi = vmulq_s32(sq_hi, w_hi);

        // Accumulate to i64 (split each i32x4 into two i64x2)
        sum0 = vaddq_s64(sum0, vmovl_s32(vget_low_s32(prod_lo)));
        sum0 = vaddq_s64(sum0, vmovl_s32(vget_high_s32(prod_lo)));
        sum1 = vaddq_s64(sum1, vmovl_s32(vget_low_s32(prod_hi)));
        sum1 = vaddq_s64(sum1, vmovl_s32(vget_high_s32(prod_hi)));
    }

    // Horizontal sum
    let total = vaddq_s64(sum0, sum1);
    (vgetq_lane_s64(total, 0) + vgetq_lane_s64(total, 1)) as i32
}

// ============================================================================
// AVX2 implementations (x86_64 only)
// ============================================================================

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn add_weights_avx2(acc: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    use std::arch::x86_64::*;

    let acc_ptr = acc.as_mut_ptr();
    let weights_ptr = weights.as_ptr();

    // Process 16 i16 values at a time (256 bits)
    for i in (0..HIDDEN_SIZE).step_by(16) {
        let a = _mm256_loadu_si256(acc_ptr.add(i) as *const __m256i);
        let w = _mm256_loadu_si256(weights_ptr.add(i) as *const __m256i);
        let sum = _mm256_adds_epi16(a, w); // Saturating add
        _mm256_storeu_si256(acc_ptr.add(i) as *mut __m256i, sum);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sub_weights_avx2(acc: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
    use std::arch::x86_64::*;

    let acc_ptr = acc.as_mut_ptr();
    let weights_ptr = weights.as_ptr();

    for i in (0..HIDDEN_SIZE).step_by(16) {
        let a = _mm256_loadu_si256(acc_ptr.add(i) as *const __m256i);
        let w = _mm256_loadu_si256(weights_ptr.add(i) as *const __m256i);
        let diff = _mm256_subs_epi16(a, w); // Saturating sub
        _mm256_storeu_si256(acc_ptr.add(i) as *mut __m256i, diff);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn screlu_dot_avx2(acc: &[i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) -> i32 {
    use std::arch::x86_64::*;

    let acc_ptr = acc.as_ptr();
    let weights_ptr = weights.as_ptr();

    let zero = _mm256_setzero_si256();
    let qa = _mm256_set1_epi16(QA as i16);

    let mut sum_lo = _mm256_setzero_si256();
    let mut sum_hi = _mm256_setzero_si256();

    for i in (0..HIDDEN_SIZE).step_by(16) {
        let a = _mm256_loadu_si256(acc_ptr.add(i) as *const __m256i);
        let w = _mm256_loadu_si256(weights_ptr.add(i) as *const __m256i);

        // Clamp to [0, QA]
        let clamped = _mm256_min_epi16(_mm256_max_epi16(a, zero), qa);

        // Unpack to i32 and square
        let lo = _mm256_unpacklo_epi16(clamped, zero);
        let hi = _mm256_unpackhi_epi16(clamped, zero);
        let sq_lo = _mm256_mullo_epi32(lo, lo);
        let sq_hi = _mm256_mullo_epi32(hi, hi);

        // Sign extend weights to i32
        let w_lo = _mm256_unpacklo_epi16(w, _mm256_cmpgt_epi16(zero, w));
        let w_hi = _mm256_unpackhi_epi16(w, _mm256_cmpgt_epi16(zero, w));

        // Multiply
        let prod_lo = _mm256_mullo_epi32(sq_lo, w_lo);
        let prod_hi = _mm256_mullo_epi32(sq_hi, w_hi);

        // Accumulate to i64
        let prod_lo_lo = _mm256_cvtepi32_epi64(_mm256_extracti128_si256(prod_lo, 0));
        let prod_lo_hi = _mm256_cvtepi32_epi64(_mm256_extracti128_si256(prod_lo, 1));
        let prod_hi_lo = _mm256_cvtepi32_epi64(_mm256_extracti128_si256(prod_hi, 0));
        let prod_hi_hi = _mm256_cvtepi32_epi64(_mm256_extracti128_si256(prod_hi, 1));

        sum_lo = _mm256_add_epi64(sum_lo, prod_lo_lo);
        sum_lo = _mm256_add_epi64(sum_lo, prod_lo_hi);
        sum_hi = _mm256_add_epi64(sum_hi, prod_hi_lo);
        sum_hi = _mm256_add_epi64(sum_hi, prod_hi_hi);
    }

    let total = _mm256_add_epi64(sum_lo, sum_hi);
    let mut result: [i64; 4] = [0; 4];
    _mm256_storeu_si256(result.as_mut_ptr() as *mut __m256i, total);

    (result[0] + result[1] + result[2] + result[3]) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_weights() {
        let mut acc = [100i16; HIDDEN_SIZE];
        let weights = [50i16; HIDDEN_SIZE];

        add_weights(&mut acc, &weights);

        for &v in &acc {
            assert_eq!(v, 150);
        }
    }

    #[test]
    fn test_sub_weights() {
        let mut acc = [100i16; HIDDEN_SIZE];
        let weights = [30i16; HIDDEN_SIZE];

        sub_weights(&mut acc, &weights);

        for &v in &acc {
            assert_eq!(v, 70);
        }
    }

    #[test]
    fn test_add_weights_saturating() {
        let mut acc = [i16::MAX - 10; HIDDEN_SIZE];
        let weights = [20i16; HIDDEN_SIZE];

        add_weights(&mut acc, &weights);

        for &v in &acc {
            assert_eq!(v, i16::MAX);
        }
    }

    #[test]
    fn test_screlu_dot_matches_scalar() {
        let acc: [i16; HIDDEN_SIZE] = std::array::from_fn(|i| (i as i16 % 300) - 50);
        let weights: [i16; HIDDEN_SIZE] = std::array::from_fn(|i| ((i as i16) % 200) - 100);

        let scalar_result = screlu_dot_scalar(&acc, &weights);
        let simd_result = screlu_dot(&acc, &weights);

        assert_eq!(
            scalar_result, simd_result,
            "SIMD result {simd_result} doesn't match scalar {scalar_result}"
        );
    }
}
