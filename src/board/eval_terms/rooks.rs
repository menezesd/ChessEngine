//! Rook activity evaluation.
//!
//! Evaluates rook placement on open files, 7th rank, and trapped rooks.

use crate::board::masks::{FILES, RANK_7TH};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use crate::board::attack_tables::slider_attacks;

use super::tables::{
    CONNECTED_ROOKS_EG, CONNECTED_ROOKS_MG, ROOK_7TH_EG, ROOK_7TH_MG, ROOK_OPEN_FILE_EG,
    ROOK_OPEN_FILE_MG, ROOK_SEMI_OPEN_EG, ROOK_SEMI_OPEN_MG, TRAPPED_ROOK_MG,
};

// File indices for trapped rook detection
const FILE_A: usize = 0;
const FILE_B: usize = 1;
const FILE_C: usize = 2;
const FILE_F: usize = 5;
const FILE_G: usize = 6;
const FILE_H: usize = 7;

impl Board {
    /// Evaluate rook activity (open files, 7th rank).
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    #[must_use]
    pub fn eval_rooks(&self) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        for color in Color::BOTH {
            let sign = color.sign();
            let our_pawns = self.pieces_of(color, Piece::Pawn);
            let enemy_pawns = self.opponent_pieces(color, Piece::Pawn);

            for sq_idx in self.pieces_of(color, Piece::Rook).iter() {
                let sq = sq_idx;
                let file = sq.file();

                // Open/semi-open file
                let file_mask = FILES[file];
                let our_pawns_on_file = (file_mask.0 & our_pawns.0) != 0;
                let enemy_pawns_on_file = (file_mask.0 & enemy_pawns.0) != 0;

                if !our_pawns_on_file {
                    if enemy_pawns_on_file {
                        // Semi-open file
                        mg += sign * ROOK_SEMI_OPEN_MG;
                        eg += sign * ROOK_SEMI_OPEN_EG;
                    } else {
                        // Open file
                        mg += sign * ROOK_OPEN_FILE_MG;
                        eg += sign * ROOK_OPEN_FILE_EG;
                    }
                }

                // Rook on 7th rank
                let seventh = RANK_7TH[color.index()];
                if (Bitboard::from_square(sq).0 & seventh.0) != 0 {
                    mg += sign * ROOK_7TH_MG;
                    eg += sign * ROOK_7TH_EG;
                }
            }

            // Trapped rook penalty
            let king_sq_idx = self.king_square_index(color);
            let king_file = king_sq_idx % 8;
            let king_rank = king_sq_idx / 8;

            // Check for trapped rook by uncastled king
            let back_rank = color.back_rank();
            if king_rank == back_rank {
                for rook_sq in self.pieces_of(color, Piece::Rook).iter() {
                    let rook_file = rook_sq.file();
                    let rook_rank = rook_sq.rank();

                    if rook_rank == back_rank {
                        // King on f/g file with rook trapped on g/h
                        if (king_file == FILE_F || king_file == FILE_G)
                            && (rook_file == FILE_G || rook_file == FILE_H)
                        {
                            mg += sign * TRAPPED_ROOK_MG;
                        }
                        // King on b/c file with rook trapped on a/b
                        if (king_file == FILE_B || king_file == FILE_C)
                            && (rook_file == FILE_A || rook_file == FILE_B)
                        {
                            mg += sign * TRAPPED_ROOK_MG;
                        }
                    }
                }
            }

            // Connected rooks bonus
            let rooks = self.pieces_of(color, Piece::Rook);
            if rooks.popcount() >= 2 {
                let mut rook_squares: [usize; 2] = [0; 2];
                let mut count = 0;
                for sq in rooks.iter() {
                    if count < 2 {
                        rook_squares[count] = sq.as_index();
                        count += 1;
                    }
                }

                if count == 2 {
                    // Check if rooks can see each other (on same rank or file with no pieces between)
                    let r1 = rook_squares[0];
                    let r2 = rook_squares[1];

                    // Get rook attacks from first rook position
                    let rook1_attacks = slider_attacks(r1, self.all_occupied.0, false);

                    // If rook 1 can attack rook 2's square, they're connected
                    if (rook1_attacks & (1u64 << r2)) != 0 {
                        mg += sign * CONNECTED_ROOKS_MG;
                        eg += sign * CONNECTED_ROOKS_EG;
                    }
                }
            }
        }

        (mg, eg)
    }
}

#[cfg(test)]
mod tests {
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
}
