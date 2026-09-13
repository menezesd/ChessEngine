use super::*;

#[test]
fn test_add_weights() {
    let mut acc = [100i32; HIDDEN_SIZE];
    let weights = [50i16; HIDDEN_SIZE];

    add_weights(&mut acc, &weights);

    for &v in &acc {
        assert_eq!(v, 150);
    }
}

#[test]
fn test_sub_weights() {
    let mut acc = [100i32; HIDDEN_SIZE];
    let weights = [30i16; HIDDEN_SIZE];

    sub_weights(&mut acc, &weights);

    for &v in &acc {
        assert_eq!(v, 70);
    }
}

#[test]
fn updates_cross_i16_limits_without_losing_information() {
    let original = i32::from(i16::MAX) - 10;
    let mut acc = [original; HIDDEN_SIZE];
    let weights = [20i16; HIDDEN_SIZE];

    add_weights(&mut acc, &weights);

    for &v in &acc {
        assert_eq!(v, original + 20);
    }
    sub_weights(&mut acc, &weights);
    assert_eq!(acc, [original; HIDDEN_SIZE]);
}

#[test]
fn activation_clamps_wide_values_and_reduces_without_overflow() {
    let weights = [i16::MAX; HIDDEN_SIZE];
    assert_eq!(screlu_dot(&[i32::MIN; HIDDEN_SIZE], &weights), 0);
    assert_eq!(
        screlu_dot(&[i32::MAX; HIDDEN_SIZE], &weights),
        HIDDEN_SIZE as i64 * i64::from(QA).pow(2) * i64::from(i16::MAX)
    );
    let alternating = std::array::from_fn(|i| if i % 2 == 0 { i16::MAX } else { i16::MIN });
    assert_eq!(
        screlu_dot(&[QA; HIDDEN_SIZE], &alternating),
        -(HIDDEN_SIZE as i64 / 2) * i64::from(QA).pow(2)
    );
}
