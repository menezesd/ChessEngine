//! Advanced evaluation terms.
//!
//! Contains evaluation functions for:
//! - Mobility (piece movement options)
//! - Pawn structure (passed, doubled, isolated, backward pawns)
//! - King safety (attack units, pawn shield)
//! - Rook activity (open files, 7th rank)
//! - Hanging pieces
//! - Drawish endgame detection
//! - Piece coordination (batteries, clusters)
//! - Advanced pawn features (storm, levers, chains)
//! - Weak squares (holes, color complexes)
//! - King danger refinements
//! - Endgame patterns (fortress, wrong bishop)
//! - Space control
//! - Advanced threats (forks, pins, skewers)
//! - Piece quality (active/passive, trapped)
//! - Imbalances (knight vs bishop, etc.)
//! - Initiative

mod combined;
mod coordination;
mod drawish;
mod endgame_patterns;
mod hanging;
pub mod helpers;
mod imbalances;
mod initiative;
mod king_danger;
mod king_safety;
mod minor_pieces;
mod mobility;
mod passed_pawns;
mod pawn_advanced;
mod pawn_structure;
mod piece_quality;
mod rooks;
mod space_control;
pub mod tables;
mod threats_advanced;
mod tropism;
mod weak_squares;

#[cfg(test)]
mod tests {
    use crate::board::state::Board;

    fn make_board(fen: &str) -> Board {
        fen.parse().expect("valid fen")
    }

    #[test]
    fn test_mobility_startpos() {
        let board = make_board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        let (mg, eg) = board.eval_mobility();
        // Should be roughly equal in starting position
        assert!(mg.abs() < 20, "mobility mg = {mg}");
        assert!(eg.abs() < 20, "mobility eg = {eg}");
    }

    #[test]
    fn test_passed_pawn() {
        // White has a passed pawn on d5
        let board = make_board("8/8/8/3P4/8/8/8/8 w - - 0 1");
        let (mg, eg) = board.eval_passed_pawns();
        assert!(mg > 0, "passed pawn mg should be positive: {mg}");
        assert!(eg > 0, "passed pawn eg should be positive: {eg}");
    }

    #[test]
    fn test_doubled_pawns() {
        // White has doubled pawns on d-file
        let board = make_board("8/8/8/3P4/3P4/8/8/8 w - - 0 1");
        let (mg, _) = board.eval_pawn_structure();
        assert!(mg < 0, "doubled pawns should be penalized: {mg}");
    }

    #[test]
    fn test_rook_open_file() {
        // White rook on open e-file
        let board = make_board("8/8/8/8/8/8/8/4R3 w - - 0 1");
        let (mg, eg) = board.eval_rooks();
        assert!(mg > 0, "rook on open file mg = {mg}");
        assert!(eg > 0, "rook on open file eg = {eg}");
    }
}
