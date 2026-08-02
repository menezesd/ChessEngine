use super::*;

#[test]
fn test_king_safety_starting_position() {
    let board: Board = Board::new();
    let (mg, eg) = board.eval_king_safety();
    assert!(mg.abs() < 50, "king safety mg should be near 0: {mg}");
    assert_eq!(eg, 0, "king safety eg should be 0");
}

#[test]
fn test_king_shield_with_pawns() {
    let board: Board = "r3k2r/ppp2ppp/8/8/8/8/PPP2PPP/R3K2R w KQkq - 0 1"
        .parse()
        .unwrap();
    let (mg, _) = board.eval_king_shield();
    assert!(mg.abs() < 30, "shield should be balanced: {mg}");
}

#[test]
fn test_king_on_open_file_penalty() {
    let board: Board = "r3k2r/pppp1ppp/8/8/8/8/PPPP1PPP/R3K2R w KQkq - 0 1"
        .parse()
        .unwrap();
    let (mg, _) = board.eval_king_shield();
    assert!(mg.abs() < 20, "both kings on open file: {mg}");
}

#[test]
fn test_castled_king_shield() {
    let board: Board = "r4rk1/ppp2ppp/8/8/8/8/PPP2PPP/R4RK1 w - - 0 1"
        .parse()
        .unwrap();
    let (mg, _) = board.eval_king_shield();
    assert!(mg.abs() < 30, "both castled with shield: {mg}");
}

#[test]
fn advanced_king_earns_no_shield_bonus_for_pawns_left_behind() {
    // Identical pawn structure; only the white king differs (e1 vs e5).
    // The e5 king must not collect a shield bonus for d2/e2/f2, so the
    // back-rank king's shield score must be strictly higher.
    let back_rank: Board = "7k/8/8/8/8/8/3PPP2/4K3 w - - 0 1".parse().unwrap();
    let advanced: Board = "7k/8/8/4K3/8/8/3PPP2/8 w - - 0 1".parse().unwrap();

    let (back_mg, _) = back_rank.eval_king_shield();
    let (adv_mg, _) = advanced.eval_king_shield();

    assert!(
        back_mg > adv_mg,
        "king on e1 should score a shield that the king on e5 does not: {back_mg} vs {adv_mg}"
    );
}
