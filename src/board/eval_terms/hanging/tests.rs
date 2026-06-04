use super::*;

#[test]
fn test_no_hanging_pieces() {
    // Starting position - all pieces defended
    let board = Board::new();
    let score = board.eval_hanging();
    // Should be roughly balanced
    assert!(score.abs() < 20, "no hanging pieces in start: {score}");
}

#[test]
fn test_hanging_knight() {
    // White knight on e4 attacked by black bishop on b7
    let board: Board = "8/1b6/8/8/4N3/8/8/8 w - - 0 1".parse().unwrap();
    let score = board.eval_hanging();
    // Hanging knight should give penalty (negative for white)
    assert!(score < 0, "hanging knight should be penalized: {score}");
}

#[test]
fn test_defended_piece_not_hanging() {
    // White knight on e4 defended by white pawn on d3
    let board: Board = "8/1b6/8/8/4N3/3P4/8/8 w - - 0 1".parse().unwrap();
    let score = board.eval_hanging();
    // Defended piece shouldn't have full hanging penalty
    // Score may still be slightly negative due to pawn attack
    assert!(
        score > -50,
        "defended piece should have less penalty: {score}"
    );
}

#[test]
fn test_minor_attacking_minor() {
    // White bishop attacks black knight
    let board: Board = "8/8/4n3/8/8/8/6B1/8 w - - 0 1".parse().unwrap();
    let score = board.eval_hanging();
    // Minor attacking minor should give slight bonus
    assert!(score >= 0, "minor on minor should be non-negative: {score}");
}

#[test]
fn test_pawn_attack_on_minor() {
    // Black pawn attacks white knight
    let board: Board = "8/8/3p4/4N3/8/8/8/8 w - - 0 1".parse().unwrap();
    let score = board.eval_hanging();
    // Pawn attacking knight is bad for knight's side
    assert!(score < 0, "pawn attacking knight should penalize: {score}");
}
