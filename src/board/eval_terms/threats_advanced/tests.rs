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
