use super::*;

#[test]
fn test_knight_outpost() {
    let board: Board = "8/8/8/3N4/2P5/8/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_minor_pieces(&ctx);
    assert!(mg > 0, "knight outpost should have positive mg: {mg}");
    assert!(eg > 0, "knight outpost should have positive eg: {eg}");
}

#[test]
fn test_knight_no_outpost_not_protected() {
    let board: Board = "8/8/8/3N4/8/8/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _) = board.eval_minor_pieces(&ctx);
    assert_eq!(mg, 0, "unprotected knight should have no outpost bonus");
}

#[test]
fn test_knight_outpost_can_be_attacked() {
    let board: Board = "8/8/2p5/3N4/2P5/8/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _) = board.eval_minor_pieces(&ctx);
    assert_eq!(mg, 0, "attackable knight should have no outpost bonus");
}

#[test]
fn test_knight_outpost_ignores_enemy_pawn_on_same_rank() {
    // The c5 pawn attacks b4/d4, not the protected knight on d5.
    let board: Board = "8/8/8/2pN4/2P5/8/8/8 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_minor_pieces(&ctx);

    assert!(mg > 0, "same-rank pawn must not cancel the outpost: {mg}");
    assert!(eg > 0, "same-rank pawn must not cancel the outpost: {eg}");
}

#[test]
fn test_bad_bishop() {
    let board: Board = "8/8/8/8/3P1P2/2P3P1/1P5P/2B5 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, eg) = board.eval_minor_pieces(&ctx);
    assert!(mg < 0, "bad bishop should have negative mg: {mg}");
    assert!(eg < 0, "bad bishop should have negative eg: {eg}");
}

#[test]
fn test_good_bishop() {
    let board: Board = "8/8/8/8/2P1P3/3P4/4P3/2B5 w - - 0 1".parse().unwrap();
    let ctx = board.compute_attack_context();
    let (mg, _) = board.eval_minor_pieces(&ctx);
    assert!(mg >= 0, "good bishop should have non-negative mg: {mg}");
}

#[test]
fn test_central_outpost_bonus() {
    let board1: Board = "8/8/8/3N4/2P5/8/8/8 w - - 0 1".parse().unwrap();
    let ctx1 = board1.compute_attack_context();
    let (mg1, _) = board1.eval_minor_pieces(&ctx1);

    let board2: Board = "8/8/8/N7/1P6/8/8/8 w - - 0 1".parse().unwrap();
    let ctx2 = board2.compute_attack_context();
    let (mg2, _) = board2.eval_minor_pieces(&ctx2);

    assert!(mg1 > mg2, "central outpost should have higher bonus");
}
