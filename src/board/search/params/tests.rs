use super::*;

#[test]
fn test_baseline_params() {
    let params = SearchParams::baseline();
    // Baseline has more aggressive pruning
    assert!(params.null_min_depth <= 3);
    assert!(params.iir_min_depth <= 4);
    assert!(params.lmp_min_depth <= 2);
}

#[test]
fn test_conservative_params() {
    let params = SearchParams::conservative();
    let baseline = SearchParams::baseline();

    // Conservative should have higher thresholds (less pruning)
    assert!(params.null_min_depth >= baseline.null_min_depth);
    assert!(params.rfp_margin >= baseline.rfp_margin);
    assert!(params.lmp_min_depth >= baseline.lmp_min_depth);
    assert!(params.lmp_move_limit >= baseline.lmp_move_limit);
}

#[test]
fn test_params_clone() {
    let params = SearchParams::baseline();
    let cloned = params.clone();
    assert_eq!(cloned.null_min_depth, params.null_min_depth);
    assert_eq!(cloned.lmr_reduction, params.lmr_reduction);
}

#[test]
fn test_null_move_params() {
    let params = SearchParams::baseline();
    // Null move should require some minimum depth
    assert!(params.null_min_depth >= 2);
    // Reduction should be reasonable
    assert!(params.null_reduction >= 1 && params.null_reduction <= 4);
}

#[test]
fn test_lmr_params() {
    let params = SearchParams::baseline();
    // LMR should start after a few moves
    assert!(params.lmr_min_move >= 2);
    // LMR should require some minimum depth
    assert!(params.lmr_min_depth >= 2);
}

#[test]
fn test_futility_margin() {
    let params = SearchParams::baseline();
    // Futility margin should be roughly pawn value
    assert!(params.futility_margin >= 50 && params.futility_margin <= 200);
}
