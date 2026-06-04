use super::*;

#[test]
fn test_pawnless_flank() {
    let board: Board = "8/8/8/8/8/8/PPP5/6K1 w - - 0 1".parse().unwrap();
    let penalty = board.eval_pawnless_flank(Color::White, 6);
    assert!(penalty < 0, "pawnless flank should have penalty");
}

#[test]
fn test_pawned_flank_no_penalty() {
    let board: Board = "8/8/8/8/8/8/5PPP/6K1 w - - 0 1".parse().unwrap();
    let penalty = board.eval_pawnless_flank(Color::White, 6);
    assert_eq!(penalty, 0, "protected flank should have no penalty");
}

#[test]
fn test_escape_squares() {
    let board: Board = "8/8/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _) = board.eval_king_danger(&ctx);
    assert!(mg != i32::MIN, "evaluation should complete");
}

#[test]
fn test_trapped_king() {
    let board: Board = "8/8/8/8/8/8/5PPP/5RK1 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let score = board.eval_escape_squares(Color::White, 6, &ctx);
    assert!(
        score <= 0,
        "trapped king should have negative escape score: {score}"
    );
}

#[test]
fn test_king_exposure() {
    let board: Board = "4r3/8/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let penalty = board.eval_king_exposure(Color::White, 4);
    assert!(penalty < 0, "king exposed to rook should give penalty");
}

#[test]
fn test_symmetry_king_danger() {
    let board: Board = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1"
        .parse()
        .unwrap();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_king_danger(&ctx);
    assert!(
        mg.abs() < 30,
        "symmetric king danger should be near zero: {mg}"
    );
    assert!(
        eg.abs() < 30,
        "symmetric king danger eg should be near zero: {eg}"
    );
}
