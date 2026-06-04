#[cfg(any(
    not(any(target_arch = "x86_64", target_arch = "aarch64")),
    all(target_arch = "x86_64", not(target_feature = "avx2"))
))]
#[inline]
pub(super) fn add_weights(
    acc: &mut [i16; super::HIDDEN_SIZE],
    weights: &[i16; super::HIDDEN_SIZE],
) {
    for i in 0..super::HIDDEN_SIZE {
        acc[i] = acc[i].saturating_add(weights[i]);
    }
}

#[cfg(any(
    not(any(target_arch = "x86_64", target_arch = "aarch64")),
    all(target_arch = "x86_64", not(target_feature = "avx2"))
))]
#[inline]
pub(super) fn sub_weights(
    acc: &mut [i16; super::HIDDEN_SIZE],
    weights: &[i16; super::HIDDEN_SIZE],
) {
    for i in 0..super::HIDDEN_SIZE {
        acc[i] = acc[i].saturating_sub(weights[i]);
    }
}

#[cfg(any(
    test,
    not(any(target_arch = "x86_64", target_arch = "aarch64")),
    all(target_arch = "x86_64", not(target_feature = "avx2"))
))]
#[inline]
pub(super) fn screlu_dot(
    acc: &[i16; super::HIDDEN_SIZE],
    weights: &[i16; super::HIDDEN_SIZE],
) -> i64 {
    let mut sum = 0i64;
    for i in 0..super::HIDDEN_SIZE {
        let clamped = i32::from(acc[i]).clamp(0, i32::from(super::QA));
        let activated = clamped * clamped;
        sum += i64::from(activated) * i64::from(weights[i]);
    }
    sum
}
