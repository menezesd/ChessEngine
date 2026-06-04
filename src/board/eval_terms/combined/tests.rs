use super::*;

#[test]
fn test_combined_eval_starting_position() {
    let board = Board::new();
    let (pass_mg, pass_eg, hanging) = board.eval_attacks_dependent();
    // Starting position has no passed pawns
    assert_eq!(pass_mg, 0, "no passed pawns in start");
    assert_eq!(pass_eg, 0, "no passed pawns in start");
    // Hanging should be roughly balanced
    assert!(hanging.abs() < 30, "hanging should be balanced: {hanging}");
}

#[test]
fn test_combined_with_passed_pawn() {
    // White has passed pawn on e6
    let board: Board = "8/4P3/8/8/8/8/8/8 w - - 0 1".parse().unwrap();
    let (pass_mg, pass_eg, _) = board.eval_attacks_dependent();
    assert!(pass_mg > 0, "passed pawn should give bonus");
    assert!(pass_eg > 0, "passed pawn should give eg bonus");
}

#[test]
fn test_context_reuse() {
    // Verify that pre-computed context gives same result
    let board = Board::new();
    let ctx = board.compute_attack_context();
    let (mg1, eg1, h1) = board.eval_attacks_dependent();
    let (mg2, eg2, h2) = board.eval_attacks_dependent_with_context(&ctx);
    assert_eq!(mg1, mg2);
    assert_eq!(eg1, eg2);
    assert_eq!(h1, h2);
}
