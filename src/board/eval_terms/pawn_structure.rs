//! Pawn structure evaluation.
//!
//! Evaluates doubled, isolated, backward pawns and phalanx/defended pawns.
//! Supports caching via pawn hash table for improved performance.

#![allow(clippy::needless_range_loop)] // 0..2 for color index is clearer

use crate::board::masks::{fill_forward, relative_rank, ADJACENT_FILES, PAWN_SUPPORT_MASK};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};
use crate::pawn_hash::PawnHashTable;

use super::tables::{
    BACKWARD_OPEN_EG, BACKWARD_OPEN_MG, BACKWARD_PAWN_EG, BACKWARD_PAWN_MG, DEFENDED_BONUS_EG,
    DEFENDED_BONUS_MG, DOUBLED_PAWN_EG, DOUBLED_PAWN_MG, ISOLATED_OPEN_EG, ISOLATED_OPEN_MG,
    ISOLATED_PAWN_EG, ISOLATED_PAWN_MG, PHALANX_BONUS_EG, PHALANX_BONUS_MG,
};

impl Board {
    /// Evaluate pawn structure.
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    #[must_use]
    pub fn eval_pawn_structure(&self) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        for color in Color::BOTH {
            let sign = color.sign();
            let color_idx = color.index();
            let own_pawns = self.pieces[color_idx][Piece::Pawn.index()];
            let enemy_pawns = self.pieces[color.opponent().index()][Piece::Pawn.index()];

            for sq_idx in own_pawns.iter() {
                let sq = sq_idx;
                let file = sq.file();
                let rank = sq.rank();
                let rel_rank = relative_rank(rank, color);

                // Doubled pawn: another own pawn ahead on same file
                let ahead = fill_forward(Bitboard::from_square(sq), color);
                let doubled = ahead.0 & own_pawns.0 & !Bitboard::from_square(sq).0;
                if doubled != 0 {
                    mg += sign * DOUBLED_PAWN_MG;
                    eg += sign * DOUBLED_PAWN_EG;
                }

                // Check for pawn support (phalanx or defended)
                let support_mask = PAWN_SUPPORT_MASK[color_idx][sq.as_index()];
                let is_supported = (support_mask.0 & own_pawns.0) != 0;

                if is_supported {
                    // Check if phalanx (side-by-side)
                    let adjacent = ADJACENT_FILES[file];
                    let same_rank = Bitboard::rank_mask(rank);
                    let phalanx = (adjacent.0 & same_rank.0 & own_pawns.0) != 0;

                    if phalanx {
                        mg += sign * PHALANX_BONUS_MG[rel_rank];
                        eg += sign * PHALANX_BONUS_EG[rel_rank];
                    } else {
                        // Defended by another pawn
                        mg += sign * DEFENDED_BONUS_MG[rel_rank];
                        eg += sign * DEFENDED_BONUS_EG[rel_rank];
                    }
                } else {
                    // Check for isolated or backward pawn
                    let adjacent_files = ADJACENT_FILES[file];
                    let has_adjacent_pawn = (adjacent_files.0 & own_pawns.0) != 0;

                    // Is file open (no enemy pawn ahead)?
                    let is_open = (ahead.0 & enemy_pawns.0) == 0;

                    if has_adjacent_pawn {
                        // Backward pawn: no pawn that can support it
                        // (has adjacent pawns but they're all ahead)
                        let behind = match color {
                            Color::White => {
                                Bitboard(fill_forward(Bitboard::from_square(sq), Color::Black).0)
                            }
                            Color::Black => {
                                Bitboard(fill_forward(Bitboard::from_square(sq), Color::White).0)
                            }
                        };
                        let support_behind = (adjacent_files.0 & behind.0 & own_pawns.0) != 0;

                        if !support_behind {
                            mg += sign * BACKWARD_PAWN_MG;
                            eg += sign * BACKWARD_PAWN_EG;
                            if is_open {
                                mg += sign * BACKWARD_OPEN_MG;
                                eg += sign * BACKWARD_OPEN_EG;
                            }
                        }
                    } else {
                        // Isolated pawn
                        mg += sign * ISOLATED_PAWN_MG;
                        eg += sign * ISOLATED_PAWN_EG;
                        if is_open {
                            mg += sign * ISOLATED_OPEN_MG;
                            eg += sign * ISOLATED_OPEN_EG;
                        }
                    }
                }
            }
        }

        (mg, eg)
    }

    /// Evaluate pawn structure with caching via pawn hash table.
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    /// Uses the pawn hash table to cache results for improved performance.
    #[must_use]
    pub fn eval_pawn_structure_cached(&self, pawn_hash_table: &PawnHashTable) -> (i32, i32) {
        let pawn_hash = self.pawn_hash();

        // Try cache first
        if let Some(entry) = pawn_hash_table.probe(pawn_hash) {
            return (entry.mg, entry.eg);
        }

        // Cache miss - compute and store
        let (mg, eg) = self.eval_pawn_structure();
        pawn_hash_table.store(pawn_hash, mg, eg);

        (mg, eg)
    }
}
