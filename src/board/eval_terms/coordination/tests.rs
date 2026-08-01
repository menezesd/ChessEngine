use super::*;

#[test]
fn test_battery_detection() {
    let board: Board = "8/8/8/8/3B4/8/1Q6/8 w - - 0 1".parse().unwrap();
    let bonus = board.eval_batteries(Color::White);
    assert!(bonus > 0, "diagonal battery should give bonus");
}

#[test]
fn test_doubled_rooks() {
    let board: Board = "8/8/8/8/4R3/8/4R3/8 w - - 0 1".parse().unwrap();
    let bonus = board.eval_batteries(Color::White);
    assert!(bonus > 0, "doubled rooks should give bonus");
}

#[test]
fn test_blocked_doubled_rooks_do_not_get_file_bonus() {
    let board: Board = "7k/8/8/8/4R3/4N3/8/4R2K w - - 0 1".parse().unwrap();

    assert_eq!(board.eval_batteries(Color::White), 0);
}

#[test]
fn test_queen_rook_battery() {
    let board: Board = "8/8/8/4Q3/8/8/4R3/8 w - - 0 1".parse().unwrap();
    let bonus = board.eval_batteries(Color::White);
    assert!(
        bonus >= BATTERY_FILE_MG,
        "Q+R file battery should give bonus"
    );
}

#[test]
fn test_no_battery_unaligned() {
    let board: Board = "8/8/8/8/3B4/8/8/1Q6 w - - 0 1".parse().unwrap();
    let bonus = board.eval_batteries(Color::White);
    assert!(
        bonus < BATTERY_DIAGONAL_MG,
        "unaligned pieces should not get full battery bonus"
    );
}

#[test]
fn test_cluster_defended_pieces() {
    let board: Board = "8/8/8/3N4/2B5/8/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_clusters(Color::White, &ctx);
    assert!(mg > 0 || eg > 0, "defended piece should give cluster bonus");
}

#[test]
fn test_full_coordination_symmetry() {
    let board: Board = "r1bqkb1r/pppppppp/2n2n2/8/8/2N2N2/PPPPPPPP/R1BQKB1R w KQkq - 0 1"
        .parse()
        .unwrap();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_coordination(&ctx);
    assert!(
        mg.abs() < 20,
        "symmetric coordination should be near zero: {mg}"
    );
    assert!(
        eg.abs() < 20,
        "symmetric coordination eg should be near zero: {eg}"
    );
}
