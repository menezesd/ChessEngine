//! Minor piece evaluation (knights and bishops).
//!
//! Evaluates:
//! - Knight outposts (knights on strong squares protected by pawns)
//! - Bishop outposts
//! - Bad bishop penalty (bishop blocked by own pawns on same color)

use crate::board::masks::ADJACENT_FILES;
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use super::helpers::AttackContext;
use super::tables::{
    BAD_BISHOP_EG, BAD_BISHOP_MG, BISHOP_OUTPOST_EG, BISHOP_OUTPOST_MG, KNIGHT_OUTPOST_EG,
    KNIGHT_OUTPOST_MG,
};

/// Outpost masks - squares that can be outposts for each color
/// An outpost is a square on ranks 4-6 (for white) that cannot be attacked by enemy pawns
const OUTPOST_RANKS: [Bitboard; 2] = [
    // White: ranks 4-6 (indices 24-47)
    Bitboard(0x0000_FFFF_FF00_0000),
    // Black: ranks 3-5 (indices 16-39)
    Bitboard(0x0000_00FF_FFFF_0000),
];

/// Central files bonus mask (c-f files get extra bonus)
const CENTRAL_FILES: Bitboard =
    Bitboard(Bitboard::FILE_C.0 | Bitboard::FILE_D.0 | Bitboard::FILE_E.0 | Bitboard::FILE_F.0);

impl Board {
    /// Evaluate minor pieces (knights and bishops).
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    #[must_use]
    pub fn eval_minor_pieces(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        for color in Color::BOTH {
            let sign = color.sign();
            let color_idx = color.index();

            // Get pawn attacks for the enemy (unused currently but kept for future use)
            let _enemy_pawn_attacks = ctx.pawn_attacks(color.opponent());
            let our_pawn_attacks = ctx.pawn_attacks(color);
            let our_pawns = self.pieces[color_idx][Piece::Pawn.index()];

            // Knight outposts
            for sq in self.pieces[color_idx][Piece::Knight.index()].iter() {
                let sq_bb = Bitboard::from_square(sq);

                // Check if on outpost rank
                if (sq_bb.0 & OUTPOST_RANKS[color_idx].0) != 0 {
                    // Check if protected by our pawn
                    if (sq_bb.0 & our_pawn_attacks.0) != 0 {
                        // Check if cannot be attacked by enemy pawns
                        let file = sq.file();
                        let adj_files = ADJACENT_FILES[file];

                        // Get enemy pawns that could attack this square
                        let enemy_pawns = self.pieces[color.opponent().index()][Piece::Pawn.index()];
                        let can_be_attacked = match color {
                            Color::White => {
                                // Enemy pawns below this square on adjacent files
                                let mask = adj_files.0
                                    & !Bitboard(0xFFFF_FFFF_FFFF_FFFF << (sq.rank() * 8)).0;
                                (enemy_pawns.0 & mask) != 0
                            }
                            Color::Black => {
                                // Enemy pawns above this square on adjacent files
                                let mask = adj_files.0
                                    & (Bitboard(0xFFFF_FFFF_FFFF_FFFF << ((sq.rank() + 1) * 8)).0);
                                (enemy_pawns.0 & mask) != 0
                            }
                        };

                        if !can_be_attacked {
                            mg += sign * KNIGHT_OUTPOST_MG;
                            eg += sign * KNIGHT_OUTPOST_EG;

                            // Extra bonus for central outposts
                            if (sq_bb.0 & CENTRAL_FILES.0) != 0 {
                                mg += sign * (KNIGHT_OUTPOST_MG / 2);
                                eg += sign * (KNIGHT_OUTPOST_EG / 2);
                            }
                        }
                    }
                }
            }

            // Bishop outposts (similar logic but smaller bonus)
            for sq in self.pieces[color_idx][Piece::Bishop.index()].iter() {
                let sq_bb = Bitboard::from_square(sq);

                if (sq_bb.0 & OUTPOST_RANKS[color_idx].0) != 0
                    && (sq_bb.0 & our_pawn_attacks.0) != 0
                {
                    let file = sq.file();
                    let adj_files = ADJACENT_FILES[file];
                    let enemy_pawns = self.pieces[color.opponent().index()][Piece::Pawn.index()];

                    let can_be_attacked = match color {
                        Color::White => {
                            let mask =
                                adj_files.0 & !Bitboard(0xFFFF_FFFF_FFFF_FFFF << (sq.rank() * 8)).0;
                            (enemy_pawns.0 & mask) != 0
                        }
                        Color::Black => {
                            let mask = adj_files.0
                                & (Bitboard(0xFFFF_FFFF_FFFF_FFFF << ((sq.rank() + 1) * 8)).0);
                            (enemy_pawns.0 & mask) != 0
                        }
                    };

                    if !can_be_attacked {
                        mg += sign * BISHOP_OUTPOST_MG;
                        eg += sign * BISHOP_OUTPOST_EG;
                    }
                }

                // Bad bishop penalty
                let sq_idx = sq.as_index();
                let is_light_square = ((sq_idx / 8) + (sq_idx % 8)) % 2 == 1;

                // Count our pawns on same color squares
                let same_color_squares = if is_light_square {
                    LIGHT_SQUARES
                } else {
                    DARK_SQUARES
                };

                let blocked_pawns = (our_pawns.0 & same_color_squares.0).count_ones() as i32;

                // Penalty scales with number of blocking pawns (3+ is bad)
                if blocked_pawns >= 3 {
                    let penalty = (blocked_pawns - 2) * BAD_BISHOP_MG;
                    mg += sign * penalty;
                    eg += sign * (blocked_pawns - 2) * BAD_BISHOP_EG;
                }
            }
        }

        (mg, eg)
    }
}

/// Light squares (a1 is dark, so b1, a2, c1, etc are light)
const LIGHT_SQUARES: Bitboard = Bitboard(0x55AA_55AA_55AA_55AA);

/// Dark squares
const DARK_SQUARES: Bitboard = Bitboard(0xAA55_AA55_AA55_AA55);
