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

    let scalar_result = scalar::screlu_dot(&acc, &weights);
    let simd_result = screlu_dot(&acc, &weights);

    assert_eq!(
        scalar_result, simd_result,
        "SIMD result {simd_result} doesn't match scalar {scalar_result}"
    );
}
