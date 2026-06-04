use super::*;

#[test]
fn test_king_safety_starting_position() {
    let board: Board = Board::new();
    let (mg, eg) = board.eval_king_safety();
    assert!(mg.abs() < 50, "king safety mg should be near 0: {mg}");
    assert_eq!(eg, 0, "king safety eg should be 0");
}

#[test]
fn test_king_shield_with_pawns() {
    let board: Board = "r3k2r/ppp2ppp/8/8/8/8/PPP2PPP/R3K2R w KQkq - 0 1"
        .parse()
        .unwrap();
    let (mg, _) = board.eval_king_shield();
    assert!(mg.abs() < 30, "shield should be balanced: {mg}");
}

#[test]
fn test_king_on_open_file_penalty() {
    let board: Board = "r3k2r/pppp1ppp/8/8/8/8/PPPP1PPP/R3K2R w KQkq - 0 1"
        .parse()
        .unwrap();
    let (mg, _) = board.eval_king_shield();
    assert!(mg.abs() < 20, "both kings on open file: {mg}");
}

#[test]
fn test_castled_king_shield() {
    let board: Board = "r4rk1/ppp2ppp/8/8/8/8/PPP2PPP/R4RK1 w - - 0 1"
        .parse()
        .unwrap();
    let (mg, _) = board.eval_king_shield();
    assert!(mg.abs() < 30, "both castled with shield: {mg}");
}
