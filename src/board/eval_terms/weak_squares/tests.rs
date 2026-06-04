use super::*;

#[test]
fn test_hole_detection() {
    let board: Board = "rnbqkb1r/pp2pppp/5n2/2pp4/3P4/5N2/PPP1PPPP/RNBQKB1R w KQkq - 0 4"
        .parse()
        .unwrap();
    let holes = board.find_holes(Color::Black);
    assert!(holes.0 != u64::MAX, "not all squares should be holes");
}

#[test]
fn test_pawn_attack_span() {
    let board: Board = "8/8/8/8/3P4/8/8/8 w - - 0 1".parse().unwrap();
    let span = board.pawn_attack_span(Color::White);
    assert!(span.0 != 0, "pawn should have attack span");
}

#[test]
fn test_weak_squares_evaluation() {
    let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        .parse()
        .unwrap();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_weak_squares(&ctx);
    assert!(mg.abs() < 30, "starting position weak squares mg: {mg}");
    assert!(eg.abs() < 30, "starting position weak squares eg: {eg}");
}

#[test]
fn test_color_weakness_no_bishop() {
    let board: Board = "8/8/8/8/8/8/PPPPPPPP/RNBQK1NR w KQ - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_color_weakness(Color::White);
    assert!(mg <= 0 && eg <= 0, "color weakness should not give bonus");
}

#[test]
fn test_weak_squares_symmetry() {
    let board = Board::new();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_weak_squares(&ctx);
    assert!(
        mg.abs() < 20,
        "symmetric weak squares should be near zero: {mg}"
    );
    assert!(
        eg.abs() < 20,
        "symmetric weak squares eg should be near zero: {eg}"
    );
}

#[test]
fn test_no_pawns_no_span() {
    let board: Board = "8/8/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let span = board.pawn_attack_span(Color::White);
    assert_eq!(span.0, 0, "no pawns should mean no attack span");
}
