use super::*;

#[test]
fn test_tropism_starting_position() {
    let board = Board::new();
    let score = board.eval_tropism();
    // Starting position should be roughly equal
    assert!(
        score.abs() < 20,
        "tropism should be near 0 in start: {score}"
    );
}

#[test]
fn test_queen_closer_to_enemy_king() {
    // White queen close to black king
    let board1: Board = "4k3/8/8/8/8/4Q3/8/4K3 w - - 0 1".parse().unwrap();
    let score1 = board1.eval_tropism();

    // White queen far from black king
    let board2: Board = "4k3/8/8/8/8/8/8/Q3K3 w - - 0 1".parse().unwrap();
    let score2 = board2.eval_tropism();

    assert!(score1 > score2, "closer queen should have higher tropism");
}

#[test]
fn test_rook_tropism() {
    // White rook close to black king
    let board: Board = "4k3/8/4R3/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let score = board.eval_tropism();
    // Rook close to enemy king should give bonus
    assert!(score > 0, "rook near enemy king should give bonus: {score}");
}

#[test]
fn test_symmetric_position() {
    // Symmetric position - both queens equidistant
    let board: Board = "4k3/8/4q3/8/8/4Q3/8/4K3 w - - 0 1".parse().unwrap();
    let score = board.eval_tropism();
    // Should be close to 0 due to symmetry
    assert!(
        score.abs() < 10,
        "symmetric position should be near 0: {score}"
    );
}
