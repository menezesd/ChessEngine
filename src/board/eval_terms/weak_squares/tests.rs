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
fn holes_lie_in_own_territory() {
    // Black has no d- or f-file pawns, so no black pawn can ever attack
    // e5: it is a hole in Black's half that a white knight could occupy.
    // Squares in White's half must never be reported as Black's holes.
    let board: Board = "4k3/pp4pp/8/8/8/8/PP4PP/4K3 w - - 0 1".parse().unwrap();
    let black_holes = board.find_holes(Color::Black);
    let e5 = 1u64 << 36;
    assert!(black_holes.0 & e5 != 0, "e5 should be a hole for Black");
    assert_eq!(
        black_holes.0 & 0x0000_0000_FFFF_FFFF,
        0,
        "Black's holes must lie in Black's half of the board"
    );

    let white_holes = board.find_holes(Color::White);
    assert_eq!(
        white_holes.0 & 0xFFFF_FFFF_0000_0000,
        0,
        "White's holes must lie in White's half of the board"
    );
}

#[test]
fn knight_on_enemy_hole_earns_occupation_bonus() {
    // White knight on e5 sits on a black hole; the same knight on e2
    // (in White's own half) must not collect the occupation bonus.
    let outpost: Board = "4k3/pp4pp/8/4N3/8/8/PP4PP/4K3 w - - 0 1".parse().unwrap();
    let home: Board = "4k3/pp4pp/8/8/8/8/PP2N1PP/4K3 w - - 0 1".parse().unwrap();

    let outpost_ctx = outpost.compute_attack_context();
    let home_ctx = home.compute_attack_context();
    let (outpost_mg, _) = outpost.eval_weak_squares(&outpost_ctx);
    let (home_mg, _) = home.eval_weak_squares(&home_ctx);

    assert!(
        outpost_mg > home_mg,
        "outpost knight should outscore home knight: {outpost_mg} vs {home_mg}"
    );
}

#[test]
fn test_no_pawns_no_span() {
    let board: Board = "8/8/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let span = board.pawn_attack_span(Color::White);
    assert_eq!(span.0, 0, "no pawns should mean no attack span");
}
