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
mod tests {
    use super::*;

    #[test]
    fn test_knight_outpost() {
        // White knight on d5 protected by pawn on c4, no black pawns on c/e files
        let board: Board = "8/8/8/3N4/2P5/8/8/8 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let (mg, eg) = board.eval_minor_pieces(&ctx);
        // Knight on outpost should have bonus
        assert!(mg > 0, "knight outpost should have positive mg: {mg}");
        assert!(eg > 0, "knight outpost should have positive eg: {eg}");
    }

    #[test]
    fn test_knight_no_outpost_not_protected() {
        // White knight on d5 but NOT protected by pawn
        let board: Board = "8/8/8/3N4/8/8/8/8 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let (mg, _) = board.eval_minor_pieces(&ctx);
        // Unprotected knight shouldn't get outpost bonus
        assert_eq!(mg, 0, "unprotected knight should have no outpost bonus");
    }

    #[test]
    fn test_knight_outpost_can_be_attacked() {
        // White knight on d5 protected by c4, but black pawn on c6 can attack
        let board: Board = "8/8/2p5/3N4/2P5/8/8/8 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let (mg, _) = board.eval_minor_pieces(&ctx);
        // Knight can be driven away - no outpost bonus
        assert_eq!(mg, 0, "attackable knight should have no outpost bonus");
    }

    #[test]
    fn test_bad_bishop() {
        // White bishop on c1 (dark square) with many pawns on dark squares
        let board: Board = "8/8/8/8/3P1P2/2P3P1/1P5P/2B5 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let (mg, eg) = board.eval_minor_pieces(&ctx);
        // Bad bishop should have penalty (negative)
        assert!(mg < 0, "bad bishop should have negative mg: {mg}");
        assert!(eg < 0, "bad bishop should have negative eg: {eg}");
    }

    #[test]
    fn test_good_bishop() {
        // White bishop on c1 (dark) with pawns on light squares
        let board: Board = "8/8/8/8/2P1P3/3P4/4P3/2B5 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let (mg, _) = board.eval_minor_pieces(&ctx);
        // Good bishop (pawns on opposite color) - no penalty
        assert!(mg >= 0, "good bishop should have non-negative mg: {mg}");
    }

    #[test]
    fn test_central_outpost_bonus() {
        // Knight on d5 (central file) vs knight on a5 (edge file)
        let board1: Board = "8/8/8/3N4/2P5/8/8/8 w - - 0 1".parse().unwrap();
        let ctx1 = board1.compute_attack_context();
        let (mg1, _) = board1.eval_minor_pieces(&ctx1);

        let board2: Board = "8/8/8/N7/1P6/8/8/8 w - - 0 1".parse().unwrap();
        let ctx2 = board2.compute_attack_context();
        let (mg2, _) = board2.eval_minor_pieces(&ctx2);

        // Central outpost should have higher bonus
        assert!(mg1 > mg2, "central outpost should have higher bonus");
    }
}
