use super::*;

#[test]
fn pack_unpack_roundtrip() {
    let test_cases = [
        (
            10u8,
            500i16,
            BoundType::Exact,
            Some(Move::from_u16(0x1234)),
            5u8,
        ),
        (255u8, -32000i16, BoundType::LowerBound, None, 63u8),
        (
            0u8,
            0i16,
            BoundType::UpperBound,
            Some(Move::from_u16(0xFFFF)),
            0u8,
        ),
    ];

    for (depth, score, bound, mv, gen) in test_cases {
        let packed = pack_entry(depth, score, bound, mv, gen);
        let unpacked = unpack_entry(packed);

        assert_eq!(unpacked.depth, depth);
        assert_eq!(unpacked.score, score);
        assert_eq!(unpacked.bound_type, bound);
        assert_eq!(unpacked.best_move.map(Move::as_u16), mv.map(Move::as_u16));
        assert_eq!(unpacked.generation, gen);
    }
}

#[test]
fn pack_unpack_extreme_values() {
    let packed = pack_entry(100, i16::MAX, BoundType::Exact, None, 30);
    let unpacked = unpack_entry(packed);
    assert_eq!(unpacked.score, i16::MAX);

    let packed = pack_entry(100, i16::MIN, BoundType::Exact, None, 30);
    let unpacked = unpack_entry(packed);
    assert_eq!(unpacked.score, i16::MIN);
}

#[test]
fn bound_type_conversion() {
    assert_eq!(
        BoundType::from_u8(BoundType::Exact.to_u8()),
        BoundType::Exact
    );
    assert_eq!(
        BoundType::from_u8(BoundType::LowerBound.to_u8()),
        BoundType::LowerBound
    );
    assert_eq!(
        BoundType::from_u8(BoundType::UpperBound.to_u8()),
        BoundType::UpperBound
    );
}
