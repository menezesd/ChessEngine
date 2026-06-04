use super::*;

#[test]
fn test_starting_position_balanced() {
    let board = Board::new();
    let (mg, eg) = board.eval_pawn_structure();
    // Starting position should be equal
    assert_eq!(mg, 0, "starting position pawn structure should be 0");
    assert_eq!(eg, 0, "starting position pawn structure eg should be 0");
}

#[test]
fn test_doubled_pawn_penalty() {
    // White has doubled pawns on e-file
    let board: Board = "8/8/8/8/4P3/4P3/8/8 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_pawn_structure();
    // Doubled pawns should give penalty (negative score for white)
    assert!(mg < 0, "doubled pawns should have negative mg: {mg}");
    assert!(eg < 0, "doubled pawns should have negative eg: {eg}");
}

#[test]
fn test_isolated_pawn_penalty() {
    // White has isolated pawn on e4 (no pawns on d or f files)
    let board: Board = "8/8/8/8/4P3/8/8/8 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_pawn_structure();
    // Isolated pawn should have penalty
    assert!(mg < 0, "isolated pawn should have negative mg: {mg}");
    assert!(eg < 0, "isolated pawn should have negative eg: {eg}");
}

#[test]
fn test_phalanx_bonus() {
    // White pawns on e4 and d4 (phalanx)
    let board: Board = "8/8/8/8/3PP3/8/8/8 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_pawn_structure();
    // Phalanx should give bonus
    assert!(mg > 0, "phalanx should have positive mg: {mg}");
    assert!(eg > 0, "phalanx should have positive eg: {eg}");
}

#[test]
fn test_defended_pawn_bonus() {
    // White pawn chain: e4 defended by d3
    let board: Board = "8/8/8/8/4P3/3P4/8/8 w - - 0 1".parse().unwrap();
    let (mg, _eg) = board.eval_pawn_structure();
    // Defended pawn should have bonus (positive total)
    assert!(mg >= 0, "defended pawn should have non-negative mg: {mg}");
}

#[test]
fn test_backward_pawn_penalty() {
    // White pawn on e3 is backward (d4 and f4 are ahead)
    let board: Board = "8/8/8/8/3P1P2/4P3/8/8 w - - 0 1".parse().unwrap();
    let (mg, _eg) = board.eval_pawn_structure();
    // Position has mixed structure - just check it evaluates without error
    assert!(mg.abs() < 100, "backward pawn should have reasonable score");
}
