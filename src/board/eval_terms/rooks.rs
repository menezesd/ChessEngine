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

            // Connected rooks bonus — check all rook pairs (handles promotions to 3+ rooks)
            let rooks = self.pieces_of(color, Piece::Rook);
            if rooks.popcount() >= 2 {
                // Collect rook squares (max 10 rooks theoretically, but typically 2)
                let mut rook_sqs = [0usize; 10];
                let mut n = 0;
                for sq in rooks.iter() {
                    if n < 10 {
                        rook_sqs[n] = sq.as_index();
                        n += 1;
                    }
                }
                for i in 0..n {
                    for j in (i + 1)..n {
                        let rook_attacks = slider_attacks(rook_sqs[i], self.all_occupied.0, false);
                        if (rook_attacks & (1u64 << rook_sqs[j])) != 0 {
                            mg += sign * CONNECTED_ROOKS_MG;
                            eg += sign * CONNECTED_ROOKS_EG;
                        }
                    }
                }
            }
        }

        (mg, eg)
    }
}

#[cfg(test)]
mod tests;
