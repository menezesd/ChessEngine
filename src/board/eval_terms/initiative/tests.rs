use super::*;

#[test]
fn test_development() {
    let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        .parse()
        .unwrap();
    let w_dev = board.eval_development(Color::White);
    let b_dev = board.eval_development(Color::Black);

    assert!(
        w_dev < 0,
        "starting position should have development penalty"
    );
    assert!(
        b_dev < 0,
        "starting position should have development penalty"
    );
}

#[test]
fn test_developed_pieces() {
    let board: Board = "r1bqkb1r/pppppppp/2n2n2/8/8/2N2N2/PPPPPPPP/R1BQKB1R w KQkq - 0 1"
        .parse()
        .unwrap();
    let w_dev = board.eval_development(Color::White);
    let start_board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        .parse()
        .unwrap();
    let w_start = start_board.eval_development(Color::White);
    assert!(w_dev > w_start, "developed pieces should have better score");
}

#[test]
fn test_tempo_threat() {
    let board: Board = "8/8/8/3n4/4B3/8/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let bonus = board.eval_tempo_threats(Color::White, &ctx);
    assert!(bonus > 0, "attacking undefended piece should give bonus");
}

#[test]
fn test_no_tempo_defended() {
    let board: Board = "8/8/4p3/3n4/8/1B6/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let bonus = board.eval_tempo_threats(Color::White, &ctx);
    assert_eq!(
        bonus, 0,
        "defended piece shouldn't give tempo bonus: {bonus}"
    );
}

#[test]
fn test_castled_bonus() {
    let board: Board = "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQK2R w KQkq - 0 1"
        .parse()
        .unwrap();
    let uncastled = board.eval_development(Color::White);

    let castled: Board = "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQ1RK1 w kq - 0 1"
        .parse()
        .unwrap();
    let castled_dev = castled.eval_development(Color::White);

    assert!(
        castled_dev > uncastled,
        "castled position should have better development score"
    );
}

#[test]
fn test_initiative_symmetry() {
    let board = Board::new();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_initiative(&ctx);
    assert!(
        mg.abs() < 20,
        "symmetric initiative should be near zero: {mg}"
    );
    assert!(
        eg.abs() < 20,
        "symmetric initiative eg should be near zero: {eg}"
    );
}
