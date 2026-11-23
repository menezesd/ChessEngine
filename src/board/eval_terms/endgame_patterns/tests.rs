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

#[test]
fn single_pawn_rook_eval_is_color_symmetric() {
    let white_pawn: Board = "4r3/R7/4P3/8/8/7k/3K4/8 w - - 0 1".parse().unwrap();
    let black_pawn: Board = "8/3k4/7K/8/8/4p3/r7/4R3 b - - 0 1".parse().unwrap();
    let white_score = white_pawn.single_pawn_rook_eval().unwrap();
    assert_eq!(black_pawn.single_pawn_rook_eval(), Some(-white_score));
    assert_eq!(white_pawn.evaluate_simple(), black_pawn.evaluate_simple());
}

#[test]
fn single_pawn_rook_eval_requires_a_rook_on_each_side() {
    let board: Board = "8/R7/4P3/8/8/7k/3K3R/8 w - - 0 1".parse().unwrap();
    assert_eq!(board.single_pawn_rook_eval(), None);
}

#[test]
fn rook_versus_pawn_accounts_for_supported_promotion_threats() {
    let slower = Board::from_fen("7k/7r/1K6/8/P7/8/8/8 w - - 0 1");
    let advanced = Board::from_fen("7k/7r/PK6/8/8/8/8/8 w - - 0 1");
    let mirrored = Board::from_fen("8/8/8/8/8/pk6/7R/7K b - - 0 1");
    let slower = slower.single_pawn_rook_eval().unwrap();
    let advanced = advanced.single_pawn_rook_eval().unwrap();
    assert!(slower < advanced);
    assert_eq!(mirrored.single_pawn_rook_eval(), Some(-advanced));
}
