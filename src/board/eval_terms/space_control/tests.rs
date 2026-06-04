use super::*;

#[test]
fn test_center_control() {
    let board: Board = "rnbqkbnr/pppppppp/8/8/3PP3/8/PPP2PPP/RNBQKBNR w KQkq - 0 1"
        .parse()
        .unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _) = board.eval_space_control(&ctx);
    assert!(mg > 0, "center pawns should give space advantage");
}

#[test]
fn test_pawn_breaks() {
    let board: Board = "8/8/3p4/2P5/8/8/8/8 w - - 0 1".parse().unwrap();
    let breaks = board.eval_pawn_breaks(Color::White);
    assert!(breaks > 0, "available capture should give pawn break bonus");
}

#[test]
fn test_no_pawn_breaks() {
    let board: Board = "8/8/8/2P5/8/8/8/8 w - - 0 1".parse().unwrap();
    let breaks = board.eval_pawn_breaks(Color::White);
    assert_eq!(breaks, 0, "no enemy pawns means no breaks");
}

#[test]
fn test_central_pawn_break_worth_more() {
    let board_center: Board = "8/8/2p5/3P4/8/8/8/8 w - - 0 1".parse().unwrap();
    let board_edge: Board = "8/8/1p6/P7/8/8/8/8 w - - 0 1".parse().unwrap();

    let center_breaks = board_center.eval_pawn_breaks(Color::White);
    let edge_breaks = board_edge.eval_pawn_breaks(Color::White);

    assert!(
        center_breaks > edge_breaks,
        "central breaks should be worth more"
    );
}

#[test]
fn test_space_symmetry() {
    let board = Board::new();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_space_control(&ctx);
    assert!(
        mg.abs() < 20,
        "starting position space should be near zero: {mg}"
    );
    assert!(
        eg.abs() < 20,
        "starting position space eg should be near zero: {eg}"
    );
}

#[test]
fn test_territory_in_enemy_half() {
    let board: Board = "8/8/3R4/8/8/8/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _) = board.eval_space_control(&ctx);
    assert!(mg > 0, "rook in enemy territory should give space bonus");
}
