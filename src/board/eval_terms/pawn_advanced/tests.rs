use super::*;

#[test]
fn test_pawn_storm() {
    let board: Board = "r1bqk2r/pppp1ppp/2n2n2/2b1p3/4P3/3P1N2/PPPB1PPP/RN1QKB1R w KQkq - 0 1"
        .parse()
        .unwrap();
    let (mg, eg) = board.eval_pawn_advanced();
    let _ = (mg, eg);
}

#[test]
fn test_chain_links() {
    let board: Board = "8/8/8/4P3/3P4/8/8/8 w - - 0 1".parse().unwrap();
    let white_pawns = board.pieces_of(Color::White, Piece::Pawn);
    let links = Board::count_chain_links(white_pawns, Color::White);
    assert!(links >= 1, "e5 should be defended by d4");
}

#[test]
fn test_no_chain() {
    let board: Board = "8/8/8/P3P3/8/8/8/8 w - - 0 1".parse().unwrap();
    let white_pawns = board.pieces_of(Color::White, Piece::Pawn);
    let links = Board::count_chain_links(white_pawns, Color::White);
    assert_eq!(links, 0, "isolated pawns should have no chain links");
}

#[test]
fn test_pawn_advanced_symmetry() {
    let board = Board::new();
    let (mg, eg) = board.eval_pawn_advanced();
    assert!(mg.abs() < 20, "starting position pawn advanced mg: {mg}");
    assert!(eg.abs() < 20, "starting position pawn advanced eg: {eg}");
}

#[test]
fn test_candidate_passer() {
    let board: Board = "8/8/3p4/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
    let (mg, eg) = board.eval_pawn_advanced();
    assert!(
        (-100..=100).contains(&mg),
        "candidate passer mg reasonable: {mg}"
    );
    assert!(
        (-100..=100).contains(&eg),
        "candidate passer eg reasonable: {eg}"
    );
}

#[test]
fn test_pawn_lever() {
    let board: Board = "4k3/8/3p4/2P5/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let (mg, _) = board.eval_pawn_advanced();
    assert!(mg >= -100, "pawn lever evaluation should work: {mg}");
}
