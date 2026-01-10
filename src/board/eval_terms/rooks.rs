//! Rook activity evaluation.
//!
//! Evaluates rook placement on open files, 7th rank, and trapped rooks.

#![allow(clippy::needless_range_loop)] // 0..2 for color index is clearer

use crate::board::masks::{FILES, RANK_7TH};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Piece};

use crate::board::attack_tables::slider_attacks;

use super::tables::{
    CONNECTED_ROOKS_EG, CONNECTED_ROOKS_MG, ROOK_7TH_EG, ROOK_7TH_MG, ROOK_OPEN_FILE_EG,
    ROOK_OPEN_FILE_MG, ROOK_SEMI_OPEN_EG, ROOK_SEMI_OPEN_MG, TRAPPED_ROOK_MG,
};

impl Board {
    /// Evaluate rook activity (open files, 7th rank).
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    #[must_use]
    pub fn eval_rooks(&self) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        for color_idx in 0..2 {
            let sign = if color_idx == 0 { 1 } else { -1 };

            let our_pawns = self.pieces[color_idx][Piece::Pawn.index()];
            let enemy_pawns = self.pieces[1 - color_idx][Piece::Pawn.index()];

            for sq_idx in self.pieces[color_idx][Piece::Rook.index()].iter() {
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
                let seventh = RANK_7TH[color_idx];
                if (Bitboard::from_square(sq).0 & seventh.0) != 0 {
                    mg += sign * ROOK_7TH_MG;
                    eg += sign * ROOK_7TH_EG;
                }
            }

            // Trapped rook penalty
            let king_bb = self.pieces[color_idx][Piece::King.index()];
            if !king_bb.is_empty() {
                let king_sq_idx = king_bb.0.trailing_zeros() as usize;
                let king_file = king_sq_idx % 8;
                let king_rank = king_sq_idx / 8;

                // Check for trapped rook by uncastled king
                let back_rank = if color_idx == 0 { 0 } else { 7 };
                if king_rank == back_rank {
                    for rook_sq in self.pieces[color_idx][Piece::Rook.index()].iter() {
                        let rook_file = rook_sq.file();
                        let rook_rank = rook_sq.rank();

                        if rook_rank == back_rank {
                            // King on f/g file with rook trapped on g/h
                            if (king_file == 5 || king_file == 6)
                                && (rook_file == 6 || rook_file == 7)
                            {
                                mg += sign * TRAPPED_ROOK_MG;
                            }
                            // King on b/c file with rook trapped on a/b
                            if (king_file == 1 || king_file == 2)
                                && (rook_file == 0 || rook_file == 1)
                            {
                                mg += sign * TRAPPED_ROOK_MG;
                            }
                        }
                    }
                }
            }

            // Connected rooks bonus
            let rooks = self.pieces[color_idx][Piece::Rook.index()];
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
