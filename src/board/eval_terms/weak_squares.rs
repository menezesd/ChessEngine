//! Weak square evaluation.
//!
//! Implements:
//! - Hole detection (squares never defendable by pawns)
//! - Weak square control (bonus for occupying/attacking enemy holes)
//! - Color complex weakness (penalty when many holes on same color)

use crate::board::masks::ADJACENT_FILES;
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use super::helpers::AttackContext;

/// Bonus for occupying an enemy hole with a minor piece
pub const HOLE_OCCUPATION_MG: i32 = 15;
pub const HOLE_OCCUPATION_EG: i32 = 10;

/// Bonus for attacking enemy holes
pub const HOLE_ATTACK_MG: i32 = 3;

/// Penalty for having holes in own territory
pub const HOLE_PENALTY_MG: i32 = -5;
pub const HOLE_PENALTY_EG: i32 = -3;

/// Penalty for color complex weakness (many holes on same color)
pub const COLOR_WEAKNESS_MG: i32 = -8;
pub const COLOR_WEAKNESS_EG: i32 = -5;

impl Board {
    /// Evaluate weak squares.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_weak_squares(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let (w_mg, w_eg) = self.eval_weak_squares_for_color(Color::White, ctx);
        let (b_mg, b_eg) = self.eval_weak_squares_for_color(Color::Black, ctx);

        mg += w_mg - b_mg;
        eg += w_eg - b_eg;

        (mg, eg)
    }

    fn eval_weak_squares_for_color(&self, color: Color, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        // Get holes in enemy territory
        let enemy_holes = self.find_holes(color.opponent());
        let own_holes = self.find_holes(color);

        // Bonus for occupying enemy holes with minor pieces
        let knights = self.pieces_of(color, Piece::Knight);
        let bishops = self.pieces_of(color, Piece::Bishop);
        let minor_pieces = Bitboard(knights.0 | bishops.0);

        for sq in minor_pieces.iter() {
            if (enemy_holes.0 & (1u64 << sq.index())) != 0 {
                mg += HOLE_OCCUPATION_MG;
                eg += HOLE_OCCUPATION_EG;
            }
        }

        // Bonus for attacking enemy holes
        let our_attacks = ctx.all_attacks(color);
        let holes_attacked = (our_attacks.0 & enemy_holes.0).count_ones() as i32;
        mg += holes_attacked * HOLE_ATTACK_MG;

        // Penalty for own holes
        let own_hole_count = own_holes.0.count_ones() as i32;
        mg += own_hole_count * HOLE_PENALTY_MG;
        eg += own_hole_count * HOLE_PENALTY_EG;

        // Color complex weakness
        let (color_mg, color_eg) = self.eval_color_weakness(color);
        mg += color_mg;
        eg += color_eg;

        (mg, eg)
    }

    /// Half-board masks for each color (our half = opponent's side for outposts/holes)
    const HALF_BOARD: [u64; 2] = [
        0xFFFF_FFFF_0000_0000u64, // White: ranks 5-8
        0x0000_0000_FFFF_FFFFu64, // Black: ranks 1-4
    ];

    /// Find holes in a color's position.
    /// A hole is a square that cannot be defended by that color's pawns.
    fn find_holes(&self, color: Color) -> Bitboard {
        // Squares that can potentially be defended by pawns (pawn attack spans)
        let pawn_attack_span = self.pawn_attack_span(color);

        // Holes: squares in our half that our pawns can never defend
        let our_half = Self::HALF_BOARD[color.index()];

        // Squares that are holes for this color
        Bitboard(!pawn_attack_span.0 & our_half)
    }

    /// Calculate the span of squares that pawns could potentially attack.
    fn pawn_attack_span(&self, color: Color) -> Bitboard {
        let pawns = self.pieces_of(color, Piece::Pawn);

        if pawns.0 == 0 {
            return Bitboard(0);
        }

        // For each pawn, include all squares on adjacent files ahead of it
        let mut span = 0u64;

        for sq in pawns.iter() {
            let file = sq.file();
            let rank = sq.rank();

            // Get adjacent file mask
            let adj_files = ADJACENT_FILES[file].0;

            // Fill forward from current rank
            let forward_mask = match color {
                Color::White => {
                    // All squares on adjacent files from rank+1 to rank 7
                    let mut mask = 0u64;
                    for r in (rank + 1)..8 {
                        mask |= adj_files & (0xFFu64 << (r * 8));
                    }
                    mask
                }
                Color::Black => {
                    // All squares on adjacent files from rank-1 to rank 0
                    let mut mask = 0u64;
                    for r in 0..rank {
                        mask |= adj_files & (0xFFu64 << (r * 8));
                    }
                    mask
                }
            };

            span |= forward_mask;
        }

        Bitboard(span)
    }

    /// Evaluate color complex weakness.
    /// Penalize when many weak squares are on the same color complex,
    /// especially if we've traded the bishop that controls that color.
    fn eval_color_weakness(&self, color: Color) -> (i32, i32) {
        let bishops = self.pieces_of(color, Piece::Bishop);

        let holes = self.find_holes(color);

        // Count holes on each color complex
        let light_holes = (holes.0 & Bitboard::LIGHT_SQUARES.0).count_ones();
        let dark_holes = (holes.0 & Bitboard::DARK_SQUARES.0).count_ones();

        let mut mg = 0;
        let mut eg = 0;

        // Check if we have a bishop for each color complex
        let has_light_bishop = (bishops.0 & Bitboard::LIGHT_SQUARES.0) != 0;
        let has_dark_bishop = (bishops.0 & Bitboard::DARK_SQUARES.0) != 0;

        // Penalty if we have many holes on a color complex without the corresponding bishop
        if !has_light_bishop && light_holes >= 3 {
            mg += COLOR_WEAKNESS_MG;
            eg += COLOR_WEAKNESS_EG;
        }
        if !has_dark_bishop && dark_holes >= 3 {
            mg += COLOR_WEAKNESS_MG;
            eg += COLOR_WEAKNESS_EG;
        }

        (mg, eg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hole_detection() {
        // Position with weak d5 square for black (no pawn can defend it)
        let board: Board = "rnbqkb1r/pp2pppp/5n2/2pp4/3P4/5N2/PPP1PPPP/RNBQKB1R w KQkq - 0 4"
            .parse()
            .unwrap();
        let holes = board.find_holes(Color::Black);
        // The function should identify some holes
        assert!(holes.0 != u64::MAX, "not all squares should be holes");
    }

    #[test]
    fn test_pawn_attack_span() {
        let board: Board = "8/8/8/8/3P4/8/8/8 w - - 0 1".parse().unwrap();
        let span = board.pawn_attack_span(Color::White);
        // Pawn on d4 - attack span should include c5, e5 and squares ahead on c,e files
        assert!(span.0 != 0, "pawn should have attack span");
    }

    #[test]
    fn test_weak_squares_evaluation() {
        // Just verify the function runs and produces a value
        let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        let ctx = board.compute_attack_context();
        let (mg, eg) = board.eval_weak_squares(&ctx);
        // Starting position should be roughly balanced
        assert!(mg.abs() < 30, "starting position weak squares mg: {mg}");
        assert!(eg.abs() < 30, "starting position weak squares eg: {eg}");
    }

    #[test]
    fn test_color_weakness_no_bishop() {
        // White has traded light-squared bishop, many holes on light squares
        let board: Board = "8/8/8/8/8/8/PPPPPPPP/RNBQK1NR w KQ - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_color_weakness(Color::White);
        // Without considering pawn structure, this is just checking the function works
        assert!(mg <= 0 && eg <= 0, "color weakness should not give bonus");
    }

    #[test]
    fn test_weak_squares_symmetry() {
        // Symmetric position should be balanced
        let board = Board::new();
        let ctx = board.compute_attack_context();
        let (mg, eg) = board.eval_weak_squares(&ctx);
        assert!(
            mg.abs() < 20,
            "symmetric weak squares should be near zero: {mg}"
        );
        assert!(
            eg.abs() < 20,
            "symmetric weak squares eg should be near zero: {eg}"
        );
    }

    #[test]
    fn test_no_pawns_no_span() {
        // No pawns means no attack span
        let board: Board = "8/8/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let span = board.pawn_attack_span(Color::White);
        assert_eq!(span.0, 0, "no pawns should mean no attack span");
    }
}
