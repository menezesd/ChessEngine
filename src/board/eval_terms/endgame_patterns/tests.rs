use super::*;

#[test]
fn test_king_centralization() {
    let bonus = Board::king_centralization_bonus(35);
    assert!(bonus > 0, "central king should have positive bonus");
}

#[test]
fn test_king_corner_less_central() {
    let corner_bonus = Board::king_centralization_bonus(0);
    let center_bonus = Board::king_centralization_bonus(35);
    assert!(
        center_bonus > corner_bonus,
        "central king should have more bonus than corner king"
    );
}

#[test]
fn test_wrong_bishop() {
    let board: Board = "8/7P/8/8/8/8/B7/8 w - - 0 1".parse().unwrap();
    let penalty = board.eval_wrong_bishop(Color::White);
    assert!(penalty < 0, "wrong bishop should have penalty");
}

#[test]
fn test_correct_bishop() {
    let board: Board = "8/7P/8/8/8/8/1B6/8 w - - 0 1".parse().unwrap();
    let penalty = board.eval_wrong_bishop(Color::White);
    assert_eq!(penalty, 0, "correct bishop should have no penalty");
}

#[test]
fn test_rook_cut_off_black_king() {
    let rooks = Bitboard(1u64 << 48);
    let bonus = Board::eval_rook_cut_off(rooks, 44, Color::White);
    assert!(bonus > 0, "rook cutting off king should give bonus");
}

#[test]
fn test_no_wrong_bishop_with_multiple_pawns() {
    let board: Board = "8/7P/8/3P4/8/8/B7/8 w - - 0 1".parse().unwrap();
    let penalty = board.eval_wrong_bishop(Color::White);
    assert_eq!(penalty, 0, "wrong bishop only applies to rook pawn only");
}

#[test]
fn test_endgame_patterns_symmetry() {
    let board: Board = "4k3/8/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_endgame_patterns();
    assert_eq!(mg, 0, "endgame patterns mg should be 0");
    assert!(
        eg.abs() < 10,
        "symmetric kings should have near-zero eg: {eg}"
    );
}
