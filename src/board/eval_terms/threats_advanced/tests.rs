use super::*;

#[test]
fn test_fork_detection() {
    let board: Board = "4k3/8/8/3N4/8/8/8/R3K3 w - - 0 1".parse().unwrap();
    let bonus = board.eval_fork_threats(Color::White);
    assert!(bonus >= 0);
}

#[test]
fn test_between_mask() {
    let mask = Board::between_mask(0, 63);
    assert!((mask & (1u64 << 9)) != 0, "b2 should be between a1 and h8");
    assert!((mask & (1u64 << 18)) != 0, "c3 should be between a1 and h8");
}

#[test]
fn test_between_mask_file() {
    let mask = Board::between_mask(0, 56);
    assert!((mask & (1u64 << 8)) != 0, "a2 should be between a1 and a8");
    assert!((mask & (1u64 << 48)) != 0, "a7 should be between a1 and a8");
}

#[test]
fn test_between_mask_rank() {
    let mask = Board::between_mask(0, 7);
    assert!((mask & (1u64 << 1)) != 0, "b1 should be between a1 and h1");
    assert!((mask & (1u64 << 6)) != 0, "g1 should be between a1 and h1");
}

#[test]
fn test_pin_evaluation() {
    let board: Board = "4k3/3n4/8/1B6/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let bonus = board.eval_pins(Color::White);
    assert!(bonus > 0, "pin should give bonus: {bonus}");
}

#[test]
fn test_pin_requires_matching_slider_alignment() {
    let board: Board = "4k3/8/8/4p3/8/8/4B3/K7 w - - 0 1".parse().unwrap();

    // A bishop on e2 cannot pin the pawn on e5 to the king on e8: they are
    // aligned on a file, not a diagonal.
    assert_eq!(board.check_pin(12, 60, true, Color::Black, true), 0);
}

#[test]
fn test_queen_pin_to_queen_is_counted() {
    let board: Board = "4k3/8/8/4q3/8/4p3/8/K3Q3 w - - 0 1".parse().unwrap();

    // White's queen pins the black pawn on e3 to the black queen on e5.
    assert_eq!(board.eval_pins(Color::White), PIN_TO_QUEEN_MG);
}

#[test]
fn test_pins_to_multiple_queens_are_all_counted() {
    let board: Board = "k7/8/8/4q2q/8/4p2p/8/K3R2R w - - 0 1".parse().unwrap();

    assert_eq!(board.eval_pins(Color::White), 2 * PIN_TO_QUEEN_MG);
}

#[test]
fn test_threats_advanced_symmetry() {
    let board: Board = "r1bqkb1r/pppppppp/2n2n2/8/8/2N2N2/PPPPPPPP/R1BQKB1R w KQkq - 0 1"
        .parse()
        .unwrap();
    let (mg, eg) = board.eval_threats_advanced();
    assert!(mg.abs() < 30, "symmetric threats should be near zero: {mg}");
    assert!(
        eg.abs() < 30,
        "symmetric threats eg should be near zero: {eg}"
    );
}

#[test]
fn test_skewer_detection() {
    let board: Board = "4k3/8/8/8/R3q3/8/8/4K3 w - - 0 1".parse().unwrap();
    let bonus = board.eval_skewers(Color::White);
    assert!(bonus >= 0, "skewer evaluation should not be negative");
}

#[test]
fn test_skewer_ignores_targets_on_unrelated_rays() {
    let board: Board = "8/8/8/4k3/8/8/8/q3R2K w - - 0 1".parse().unwrap();

    // The rook checks the king on e5, but the queen on a1 is not behind that
    // king on the e-file and therefore is not a skewer target.
    assert!(!board.is_skewer(4, 36, 1u64, false));
}

#[test]
fn test_enemy_blocker_prevents_discovery_potential() {
    // The knight on d2 can expose Bc1-h6. An extra black pawn on e3 still
    // blocks that diagonal, so moving the knight cannot discover a check.
    let open: Board = "8/8/7k/8/8/8/3N4/K1B5 w - - 0 1".parse().unwrap();
    let blocked: Board = "8/8/7k/8/8/4p3/3N4/K1B5 w - - 0 1".parse().unwrap();

    assert_eq!(
        open.eval_discovery_potential(Color::White),
        DISCOVERY_POTENTIAL_MG
    );
    assert_eq!(blocked.eval_discovery_potential(Color::White), 0);
}
