use super::*;

#[test]
fn test_trapped_piece() {
    let board: Board = "8/8/1p6/p7/N7/8/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _eg) = board.eval_piece_quality(&ctx);
    assert!(mg < 10, "trapped knight should have penalty: {mg}");
}

#[test]
fn test_active_piece() {
    let board: Board = "8/8/8/3N4/8/8/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _) = board.eval_piece_quality(&ctx);
    assert!(mg >= 0, "central knight should not have penalty");
}

#[test]
fn test_bishop_activity() {
    let board: Board = "8/8/8/8/8/8/6B1/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _) = board.eval_piece_quality(&ctx);
    assert!(mg >= 0, "active bishop should not have penalty");
}

#[test]
fn test_piece_quality_symmetry() {
    let board: Board = "r1bqkb1r/pppppppp/2n2n2/8/8/2N2N2/PPPPPPPP/R1BQKB1R w KQkq - 0 1"
        .parse()
        .unwrap();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_piece_quality(&ctx);
    assert!(
        mg.abs() < 30,
        "symmetric piece quality should be near zero: {mg}"
    );
    assert!(
        eg.abs() < 30,
        "symmetric piece quality eg should be near zero: {eg}"
    );
}

#[test]
fn test_rook_activity() {
    let board: Board = "8/8/8/8/8/8/8/4R3 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _) = board.eval_piece_quality(&ctx);
    assert!(mg >= 0, "rook on open file should not have penalty");
}
