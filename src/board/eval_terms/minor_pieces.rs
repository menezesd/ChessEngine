//! Minor piece evaluation (knights and bishops).
//!
//! Evaluates:
//! - Knight outposts (knights on strong squares protected by pawns)
//! - Bishop outposts
//! - Bad bishop penalty (bishop blocked by own pawns on same color)

use crate::board::masks::ADJACENT_FILES;
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece, Square};

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

/// Check if a square is a protected outpost (on outpost rank, protected by pawn,
/// cannot be attacked by enemy pawns).
fn is_protected_outpost(
    sq: Square,
    color: Color,
    our_pawn_attacks: Bitboard,
    enemy_pawns: Bitboard,
) -> bool {
    let sq_bb = Bitboard::from_square(sq);
    let color_idx = color.index();

    // Must be on outpost rank
    if sq_bb.is_disjoint(OUTPOST_RANKS[color_idx]) {
        return false;
    }

    // Must be protected by our pawn
    if sq_bb.is_disjoint(our_pawn_attacks) {
        return false;
    }

    // Check if can be attacked by enemy pawns on adjacent files
    // For White outposts: check for Black pawns ABOVE (they attack downward)
    // For Black outposts: check for White pawns BELOW (they attack upward)
    let file = sq.file();
    let adj_files = ADJACENT_FILES[file];

    let can_be_attacked = match color {
        Color::White => {
            // Black pawns above this square can attack it (pawns attack diagonally forward)
            let mask = Bitboard(adj_files.0 & (u64::MAX << (sq.rank() * 8)));
            enemy_pawns.intersects(mask)
        }
        Color::Black => {
            // White pawns below this square can attack it
            let mask = Bitboard(adj_files.0 & !(u64::MAX << ((sq.rank() + 1) * 8)));
            enemy_pawns.intersects(mask)
        }
    };

    !can_be_attacked
}

impl Board {
    /// Evaluate minor pieces (knights and bishops).
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    #[must_use]
    pub fn eval_minor_pieces(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        for color in Color::BOTH {
            let sign = color.sign();

            let our_pawn_attacks = ctx.pawn_attacks(color);
            let our_pawns = self.pieces_of(color, Piece::Pawn);
            let enemy_pawns = self.opponent_pieces(color, Piece::Pawn);

            // Knight outposts
            for sq in self.pieces_of(color, Piece::Knight).iter() {
                if is_protected_outpost(sq, color, our_pawn_attacks, enemy_pawns) {
                    mg += sign * KNIGHT_OUTPOST_MG;
                    eg += sign * KNIGHT_OUTPOST_EG;

                    // Extra bonus for central outposts
                    let sq_bb = Bitboard::from_square(sq);
                    if sq_bb.intersects(CENTRAL_FILES) {
                        mg += sign * (KNIGHT_OUTPOST_MG / 2);
                        eg += sign * (KNIGHT_OUTPOST_EG / 2);
                    }
                }
            }

            // Bishop outposts (similar logic but smaller bonus)
            for sq in self.pieces_of(color, Piece::Bishop).iter() {
                if is_protected_outpost(sq, color, our_pawn_attacks, enemy_pawns) {
                    mg += sign * BISHOP_OUTPOST_MG;
                    eg += sign * BISHOP_OUTPOST_EG;
                }

                // Bad bishop penalty
                let sq_idx = sq.as_index();
                let is_light_square = ((sq_idx / 8) + (sq_idx % 8)) % 2 == 1;

                // Count our pawns on same color squares
                let same_color_squares = if is_light_square {
                    Bitboard::LIGHT_SQUARES
                } else {
                    Bitboard::DARK_SQUARES
                };

                let blocked_pawns = our_pawns.intersect_popcount(same_color_squares) as i32;

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

#[cfg(test)]
mod tests;
