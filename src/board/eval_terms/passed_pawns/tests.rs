use super::*;

#[test]
fn test_is_passed_pawn() {
    // White pawn on e5, no black pawns on d/e/f files ahead
    let board: Board = "8/8/8/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
    let e5 = Square::new(4, 4); // rank 4 (0-indexed), file e
    assert!(board.is_passed_pawn(e5, Color::White));
}

#[test]
fn test_is_not_passed_blocked() {
    // White pawn on e5, black pawn on e6 blocks it
    let board: Board = "8/8/4p3/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
    let e5 = Square::new(4, 4);
    assert!(!board.is_passed_pawn(e5, Color::White));
}

#[test]
fn test_is_not_passed_adjacent() {
    // White pawn on e5, black pawn on d6 guards promotion path
    let board: Board = "8/8/3p4/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
    let e5 = Square::new(4, 4);
    assert!(!board.is_passed_pawn(e5, Color::White));
}

#[test]
fn test_passed_pawn_bonus() {
    // White has passed pawn on 6th rank (very advanced)
    let board: Board = "8/4P3/8/8/8/8/8/8 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_passed_pawns();
    // Advanced passed pawn should have significant bonus
    assert!(mg > 0, "passed pawn mg should be positive: {mg}");
    assert!(eg > 0, "passed pawn eg should be positive: {eg}");
}

#[test]
fn test_no_passed_pawns() {
    // Starting position - no passed pawns
    let board = Board::new();
    let (mg, eg) = board.eval_passed_pawns();
    assert_eq!(mg, 0, "no passed pawns in starting position");
    assert_eq!(eg, 0, "no passed pawns in starting position");
}

#[test]
fn test_rook_behind_passer() {
    // White passed pawn on e5 with rook behind on e1
    let board: Board = "8/8/8/4P3/8/8/8/4R3 w - - 0 1".parse().unwrap();
    let (mg1, eg1) = board.eval_passed_pawns();

    // Compare to pawn without rook support
    let board2: Board = "8/8/8/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
    let (mg2, eg2) = board2.eval_passed_pawns();

    // Rook behind passer should give bonus
    assert!(mg1 > mg2, "rook behind passer should add bonus");
    assert!(eg1 > eg2, "rook behind passer should add eg bonus");
}

#[test]
fn test_blocked_rook_does_not_support_passer() {
    let open: Board = "7k/8/8/4P3/8/2B5/8/4R2K w - - 0 1".parse().unwrap();
    let blocked: Board = "7k/8/8/4P3/8/4B3/8/4R2K w - - 0 1".parse().unwrap();

    let (open_mg, open_eg) = open.eval_passed_pawns();
    let (blocked_mg, blocked_eg) = blocked.eval_passed_pawns();

    assert!(open_mg > blocked_mg, "open={open_mg}, blocked={blocked_mg}");
    assert!(open_eg > blocked_eg, "open={open_eg}, blocked={blocked_eg}");
}
