use super::*;

#[test]
fn bucket_count_uses_default_for_zero_size() {
    assert_eq!(
        pawn_bucket_count(0, std::mem::size_of::<PawnBucket>()),
        DEFAULT_BUCKETS
    );
}

#[test]
fn bucket_count_matches_existing_rounding_for_normal_size() {
    let bucket_size = std::mem::size_of::<PawnBucket>();
    let requested_buckets = (64 * BYTES_PER_KIB) / bucket_size;
    let expected = requested_buckets.next_power_of_two() / 2;

    assert_eq!(pawn_bucket_count(64, bucket_size), expected);
}

#[test]
fn bucket_count_saturates_extreme_size_arithmetic() {
    let count = pawn_bucket_count(usize::MAX, std::mem::size_of::<PawnBucket>());

    assert!(count.is_power_of_two());
    assert!(count >= DEFAULT_BUCKETS);
}

#[test]
fn test_pack_unpack_roundtrip() {
    let test_cases = [(100, 200), (-500, 300), (0, 0), (i32::MAX, i32::MIN)];

    for (mg, eg) in test_cases {
        let packed = pack_entry(mg, eg);
        let unpacked = unpack_entry(packed);
        assert_eq!(unpacked.mg, mg);
        assert_eq!(unpacked.eg, eg);
    }
}

#[test]
fn test_store_and_probe() {
    let table = PawnHashTable::new(64);
    let hash = 0x123456789ABCDEF0;

    table.store(hash, 150, -50);

    let entry = table.probe(hash).expect("should find entry");
    assert_eq!(entry.mg, 150);
    assert_eq!(entry.eg, -50);
}

#[test]
fn test_store_and_probe_zero_hash_and_score() {
    let table = PawnHashTable::new(64);

    table.store(0, 0, 0);

    let entry = table.probe(0).expect("zero-valued entry should be cached");
    assert_eq!(entry.mg, 0);
    assert_eq!(entry.eg, 0);
}

#[test]
fn test_no_false_positives() {
    let table = PawnHashTable::new(64);
    let hash1 = 0x123456789ABCDEF0;
    let hash2 = 0xFEDCBA9876543210;

    table.store(hash1, 100, 200);

    assert!(table.probe(hash2).is_none());
}

#[test]
fn test_update_existing() {
    let table = PawnHashTable::new(64);
    let hash = 0x123456789ABCDEF0;

    table.store(hash, 100, 200);
    table.store(hash, 300, 400);

    let entry = table.probe(hash).expect("should find entry");
    assert_eq!(entry.mg, 300);
    assert_eq!(entry.eg, 400);
}

#[test]
fn test_clear() {
    let table = PawnHashTable::new(64);
    let hash = 0x123456789ABCDEF0;

    table.store(hash, 100, 200);
    assert!(table.probe(hash).is_some());

    table.clear();
    assert!(table.probe(hash).is_none());
}
