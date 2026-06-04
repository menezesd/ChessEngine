use super::*;

#[test]
fn bucket_count_uses_default_for_zero_size() {
    assert_eq!(
        tt_bucket_count(0, std::mem::size_of::<TTBucket>()),
        DEFAULT_BUCKETS
    );
}

#[test]
fn bucket_count_matches_existing_rounding_for_normal_size() {
    let bucket_size = std::mem::size_of::<TTBucket>();
    let requested_buckets = BYTES_PER_MIB / bucket_size;
    let expected = requested_buckets.next_power_of_two() / 2;

    assert_eq!(tt_bucket_count(1, bucket_size), expected);
}

#[test]
fn bucket_count_saturates_extreme_size_arithmetic() {
    let count = tt_bucket_count(usize::MAX, std::mem::size_of::<TTBucket>());

    assert!(count.is_power_of_two());
    assert!(count >= DEFAULT_BUCKETS);
}

#[test]
fn test_store_and_probe() {
    let tt = TranspositionTable::new(1);
    let hash = 0x123456789ABCDEF0;

    tt.store(hash, 10, 500, BoundType::Exact, None, 1);

    let entry = tt.probe(hash).expect("should find entry");
    assert_eq!(entry.depth, 10);
    assert_eq!(entry.score, 500);
    assert_eq!(entry.bound_type, BoundType::Exact);
}

#[test]
fn test_no_false_positives() {
    let tt = TranspositionTable::new(1);
    let hash1 = 0x123456789ABCDEF0;
    let hash2 = 0xFEDCBA9876543210;

    tt.store(hash1, 10, 500, BoundType::Exact, None, 1);

    assert!(tt.probe(hash2).is_none());
}

#[test]
fn test_store_with_move() {
    let tt = TranspositionTable::new(1);
    let hash = 0xABCDEF0123456789;
    let mv = Move::from_u16(0x1234);

    tt.store(hash, 5, 100, BoundType::LowerBound, Some(mv), 1);

    let entry = tt.probe(hash).expect("should find entry");
    assert_eq!(entry.best_move.map(Move::as_u16), Some(0x1234));
}

#[test]
fn test_store_overwrites_same_hash() {
    let tt = TranspositionTable::new(1);
    let hash = 0x123456789ABCDEF0;

    tt.store(hash, 5, 100, BoundType::Exact, None, 1);
    tt.store(hash, 10, 200, BoundType::LowerBound, None, 1);

    let entry = tt.probe(hash).expect("should find entry");
    assert_eq!(entry.depth, 10);
    assert_eq!(entry.score, 200);
}

#[test]
fn test_multiple_entries_different_hashes() {
    let tt = TranspositionTable::new(1);

    let hash1 = 0x1111111111111111;
    let hash2 = 0x2222222222222222;
    let hash3 = 0x3333333333333333;

    tt.store(hash1, 5, 100, BoundType::Exact, None, 1);
    tt.store(hash2, 10, 200, BoundType::LowerBound, None, 1);
    tt.store(hash3, 15, 300, BoundType::UpperBound, None, 1);

    let entry1 = tt.probe(hash1).expect("should find entry1");
    assert_eq!(entry1.score, 100);

    let entry2 = tt.probe(hash2).expect("should find entry2");
    assert_eq!(entry2.score, 200);

    let entry3 = tt.probe(hash3).expect("should find entry3");
    assert_eq!(entry3.score, 300);
}

#[test]
fn test_deeper_entry_preferred() {
    let tt = TranspositionTable::new(1);
    let hash = 0x123456789ABCDEF0;

    tt.store(hash, 3, 100, BoundType::Exact, None, 1);
    tt.store(hash, 10, 500, BoundType::Exact, None, 1);

    let entry = tt.probe(hash).expect("should find entry");
    assert_eq!(entry.depth, 10);
}

#[test]
fn test_newer_generation_preferred() {
    let tt = TranspositionTable::new(1);

    for i in 0..4 {
        let hash = 0x1000000000000000u64 + i;
        tt.store(hash, 5, 100, BoundType::Exact, None, 0);
    }

    let new_hash = 0x1000000000000004u64;
    tt.store(new_hash, 5, 200, BoundType::Exact, None, 10);

    let entry = tt.probe(new_hash).expect("should find new entry");
    assert_eq!(entry.score, 200);
}

#[test]
fn test_clear() {
    let tt = TranspositionTable::new(1);
    let hash = 0x123456789ABCDEF0;

    tt.store(hash, 10, 500, BoundType::Exact, None, 1);
    assert!(tt.probe(hash).is_some());

    tt.clear();
    assert!(tt.probe(hash).is_none());
}

#[test]
fn test_hashfull_empty() {
    let tt = TranspositionTable::new(1);
    assert_eq!(tt.hashfull_per_mille(), 0);
}

#[test]
fn test_hashfull_increases() {
    let tt = TranspositionTable::new(1);

    for i in 0..100 {
        tt.store(i as u64 * 0x123456789, 5, 100, BoundType::Exact, None, 1);
    }

    let hashfull = tt.hashfull_per_mille();
    assert!(hashfull > 0, "hashfull should be > 0 after storing entries");
}

#[test]
fn test_score_clamping() {
    let tt = TranspositionTable::new(1);
    let hash = 0x123456789ABCDEF0;

    tt.store(hash, 10, 100000, BoundType::Exact, None, 1);

    let entry = tt.probe(hash).expect("should find entry");
    assert_eq!(entry.score, i16::MAX);
}

#[test]
fn test_depth_clamping() {
    let tt = TranspositionTable::new(1);
    let hash = 0x123456789ABCDEF0;

    tt.store(hash, 1000, 500, BoundType::Exact, None, 1);

    let entry = tt.probe(hash).expect("should find entry");
    assert_eq!(entry.depth, 255);
}

#[test]
fn test_entry_accessors() {
    let entry = TTEntry {
        depth: 10,
        score: 500,
        bound_type: BoundType::Exact,
        best_move: Some(Move::from_u16(0x1234)),
        generation: 5,
    };

    assert_eq!(entry.depth(), 10);
    assert_eq!(entry.score(), 500);
    assert_eq!(entry.bound_type(), BoundType::Exact);
    assert!(entry.best_move().is_some());
}

#[test]
fn test_zero_hash() {
    let tt = TranspositionTable::new(1);

    tt.store(0, 10, 500, BoundType::Exact, None, 1);
}

#[test]
fn test_all_ones_hash() {
    let tt = TranspositionTable::new(1);
    let hash = u64::MAX;

    tt.store(hash, 10, 500, BoundType::Exact, None, 1);

    let entry = tt.probe(hash).expect("should find entry");
    assert_eq!(entry.score, 500);
}

#[test]
fn test_generation_wraps() {
    let tt = TranspositionTable::new(1);
    let hash = 0x123456789ABCDEF0;

    tt.store(hash, 10, 500, BoundType::Exact, None, 63);
    let entry = tt.probe(hash).expect("should find entry");
    assert_eq!(entry.generation, 63);

    tt.store(hash, 10, 600, BoundType::Exact, None, 100);
    let entry = tt.probe(hash).expect("should find entry");
    assert_eq!(entry.generation, 36);
}
