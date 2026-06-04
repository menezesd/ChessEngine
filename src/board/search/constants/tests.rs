use super::*;

#[test]
fn test_score_bounds() {
    // SCORE_INFINITE should be the highest
    assert!(SCORE_INFINITE > SCORE_SAFE_MAX);
    assert!(SCORE_SAFE_MAX > MATE_THRESHOLD);
    // MATE_THRESHOLD is for detecting checkmate scores
    assert!(MATE_THRESHOLD > SCORE_NEAR_MATE);
}

#[test]
fn test_mate_threshold() {
    // Mate threshold should distinguish mate from normal scores
    assert!(MATE_THRESHOLD > 10000);
    assert!(MATE_THRESHOLD < SCORE_INFINITE);
}

#[test]
fn test_move_ordering_priorities() {
    // TT move should be highest priority
    assert!(TT_MOVE_SCORE > CAPTURE_BASE_SCORE);
    // Captures should beat killers
    assert!(CAPTURE_BASE_SCORE > KILLER1_SCORE);
    // Killer ordering
    assert!(KILLER1_SCORE > KILLER2_SCORE);
    assert!(KILLER2_SCORE > KILLER3_SCORE);
    assert!(KILLER3_SCORE > COUNTER_SCORE);
}

#[test]
fn test_lmr_threshold() {
    // LMR threshold should be between counter and killer scores
    assert!(LMR_SCORE_THRESHOLD < COUNTER_SCORE);
    assert!(LMR_SCORE_THRESHOLD > 0);
}

#[test]
fn test_lmr_table_dimensions() {
    // LMR table should have reasonable dimensions
    assert!(LMR_TABLE_MAX_DEPTH >= 32);
    assert!(LMR_TABLE_MAX_IDX >= 64);
}

#[test]
fn test_pawn_extension_ranks() {
    // White pre-promotion rank is 7th rank (index 6)
    assert_eq!(PAWN_EXTENSION_RANK_WHITE, 6);
    // Black pre-promotion rank is 2nd rank (index 1)
    assert_eq!(PAWN_EXTENSION_RANK_BLACK, 1);
}

#[test]
fn test_qsearch_depth() {
    // Qsearch should have reasonable max depth
    assert!(MAX_QSEARCH_DEPTH >= 8);
    assert!(MAX_QSEARCH_DEPTH <= 20);
}
