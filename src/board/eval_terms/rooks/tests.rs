use super::*;

#[test]
fn test_rook_on_open_file() {
    // White rook on open e-file
    let board: Board = "8/pppp1ppp/8/8/8/8/PPPP1PPP/4R3 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_rooks();
    // Open file bonus should make score positive for white
    assert!(mg > 0, "rook on open file should give mg bonus: {mg}");
    assert!(eg > 0, "rook on open file should give eg bonus: {eg}");
}

#[test]
fn test_rook_on_semi_open_file() {
    // White rook on semi-open e-file (no white pawn, has black pawn)
    let board: Board = "8/pppppppp/8/8/8/8/PPPP1PPP/4R3 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_rooks();
    // Semi-open file bonus (smaller than open file)
    assert!(mg > 0, "rook on semi-open file should give bonus: {mg}");
    assert!(eg > 0, "rook on semi-open file should give eg bonus: {eg}");
}

#[test]
fn test_rook_on_7th_rank() {
    // White rook on 7th rank
    let board: Board = "8/R7/8/8/8/8/8/8 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_rooks();
    // 7th rank bonus
    assert!(mg > 0, "rook on 7th rank should give bonus: {mg}");
    assert!(eg > 0, "rook on 7th rank should give eg bonus: {eg}");
}

#[test]
fn test_connected_rooks() {
    // Two white rooks on same rank (connected)
    let board: Board = "8/8/8/8/8/8/8/R6R w - - 0 1".parse().unwrap();
    let (mg1, eg1) = board.eval_rooks();

    // Two white rooks not connected (piece between)
    let board2: Board = "8/8/8/8/8/8/8/R3N2R w - - 0 1".parse().unwrap();
    let (mg2, eg2) = board2.eval_rooks();

    // Connected rooks should have higher bonus
    assert!(mg1 > mg2, "connected rooks should have higher mg bonus");
    assert!(eg1 > eg2, "connected rooks should have higher eg bonus");
}

#[test]
fn test_trapped_rook() {
    // White king on g1 with rook trapped on h1 (can't castle)
    let board: Board = "8/8/8/8/8/8/8/5RKR w - - 0 1".parse().unwrap();
    let (mg, _) = board.eval_rooks();
    // Should have trapped rook penalty (negative or less positive)
    // Note: also has open file bonuses, so check relative
    let board2: Board = "8/8/8/8/8/8/8/R4RK1 w - - 0 1".parse().unwrap();
    let (mg2, _) = board2.eval_rooks();
    assert!(
        mg < mg2,
        "trapped rook should have penalty vs castled position"
    );
}
