use super::*;

#[test]
fn test_bishop_pair_bonus() {
    let board: Board = "8/8/8/8/8/8/8/2BB4 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_imbalances_for_color(Color::White);
    assert!(mg > 0 || eg > 0, "bishop pair should have bonus");
}

#[test]
fn test_knight_closed_position() {
    let board: Board = "8/pppppppp/8/8/8/8/PPPPPPPP/2N5 w - - 0 1".parse().unwrap();
    let (mg, _) = board.eval_imbalances_for_color(Color::White);
    assert!(mg >= 0, "knight in closed position should not be penalized");
}

#[test]
fn test_bishop_open_position() {
    let board: Board = "8/8/8/8/8/8/8/2B5 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_imbalances_for_color(Color::White);
    assert!(
        mg >= 0 || eg >= 0,
        "bishop in open position should not be penalized"
    );
}

#[test]
fn test_knight_pair_penalty() {
    let board: Board = "8/8/8/8/8/8/8/2NN4 w - - 0 1".parse().unwrap();
    let (mg, _) = board.eval_imbalances_for_color(Color::White);
    assert!(mg < 0, "knight pair should have penalty: {mg}");
}

#[test]
fn test_rook_pair_vs_queen() {
    let board: Board = "4q3/8/8/8/8/8/8/R3R3 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_imbalances();
    assert!(
        mg >= 0 || eg >= 0,
        "rook pair vs queen should be reasonable"
    );
}

#[test]
fn test_imbalances_symmetry() {
    let board = Board::new();
    let (mg, eg) = board.eval_imbalances();
    assert!(mg.abs() < 20, "symmetric imbalances mg: {mg}");
    assert!(eg.abs() < 20, "symmetric imbalances eg: {eg}");
}

#[test]
fn test_bishop_advantage_open() {
    let board: Board = "8/8/8/8/8/8/8/2B2n2 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_imbalances();
    assert!((-50..=50).contains(&mg), "bishop vs knight mg: {mg}");
    assert!((-50..=50).contains(&eg), "bishop vs knight eg: {eg}");
}
