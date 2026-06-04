use super::*;

#[test]
fn test_mobility_starting_position() {
    let board = Board::new();
    let (mg, eg) = board.eval_mobility();
    // Starting position should be roughly equal
    assert!(mg.abs() < 20, "mobility mg should be near 0: {mg}");
    assert!(eg.abs() < 20, "mobility eg should be near 0: {eg}");
}

#[test]
fn test_knight_center_more_mobile() {
    // Knight in center (e4) has more squares
    let board1: Board = "8/8/8/8/4N3/8/8/8 w - - 0 1".parse().unwrap();
    let (mg1, _) = board1.eval_mobility();

    // Knight in corner (a1) has fewer squares
    let board2: Board = "8/8/8/8/8/8/8/N7 w - - 0 1".parse().unwrap();
    let (mg2, _) = board2.eval_mobility();

    assert!(mg1 > mg2, "center knight should have more mobility");
}

#[test]
fn test_bishop_on_open_diagonal() {
    // Bishop on long diagonal with few blockers
    let board: Board = "8/8/8/8/8/8/8/B7 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_mobility();
    // Open bishop should have positive mobility
    assert!(mg > 0, "open bishop should have positive mobility: {mg}");
    assert!(eg > 0, "open bishop should have positive eg mobility: {eg}");
}

#[test]
fn test_rook_more_mobile_on_open_board() {
    // Rook on empty board
    let board1: Board = "8/8/8/8/4R3/8/8/8 w - - 0 1".parse().unwrap();
    let (mg1, _) = board1.eval_mobility();

    // Rook blocked by pawns
    let board2: Board = "8/8/8/4P3/4R3/4P3/8/8 w - - 0 1".parse().unwrap();
    let (mg2, _) = board2.eval_mobility();

    assert!(mg1 > mg2, "rook on open board should have more mobility");
}

#[test]
fn test_queen_mobility() {
    // Queen in center with open lines
    let board: Board = "8/8/8/8/4Q3/8/8/8 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_mobility();
    // Queen should have high mobility score
    assert!(mg > 0, "queen should have positive mobility: {mg}");
    assert!(eg > 0, "queen should have positive eg mobility: {eg}");
}
