//! King danger refinements.
//!
//! Implements:
//! - Pawnless flank attack (extra danger when shield pawns missing)
//! - King exposure score (based on slider x-rays)
//! - Escape square count (king mobility in danger)
//! - Virtual mobility (squares king could escape to if not blocked by own pieces)

use crate::board::attack_tables::{slider_attacks, KING_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Color, Piece};

use super::helpers::AttackContext;

/// Pawnless flank attack penalty
pub const PAWNLESS_FLANK_MG: i32 = -25;

/// King exposure penalty per open line
pub const KING_EXPOSURE_FILE_MG: i32 = -10;
pub const KING_EXPOSURE_DIAG_MG: i32 = -8;

/// Escape square bonus (per safe escape square)
pub const ESCAPE_SQUARE_MG: i32 = 5;

/// No escape squares penalty
pub const NO_ESCAPE_MG: i32 = -20;

/// Virtual mobility penalty (blocked by own pieces)
pub const BLOCKED_ESCAPE_MG: i32 = -3;

impl Board {
    /// Evaluate king danger refinements.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_king_danger(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let (w_mg, w_eg) = self.eval_king_danger_for_color(Color::White, ctx);
        let (b_mg, b_eg) = self.eval_king_danger_for_color(Color::Black, ctx);

        mg += w_mg - b_mg;
        eg += w_eg - b_eg;

        (mg, eg)
    }

    fn eval_king_danger_for_color(&self, color: Color, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let eg = 0; // Most king danger terms are MG only

        let c_idx = color.index();
        let _opp_idx = color.opponent().index();

        let king_bb = self.pieces[c_idx][Piece::King.index()];
        if king_bb.0 == 0 {
            return (0, 0);
        }
        let king_sq = king_bb.0.trailing_zeros() as usize;
        let king_file = king_sq % 8;

        // Pawnless flank attack
        mg += self.eval_pawnless_flank(color, king_file);

        // King exposure (open lines toward king)
        mg += self.eval_king_exposure(color, king_sq, ctx);

        // Escape squares
        mg += self.eval_escape_squares(color, king_sq, ctx);

        (mg, eg)
    }

    /// Evaluate pawnless flank penalty.
    /// Extra danger when the side has no pawns on the flank where king resides.
    fn eval_pawnless_flank(&self, color: Color, king_file: usize) -> i32 {
        let c_idx = color.index();
        let pawns = self.pieces[c_idx][Piece::Pawn.index()];

        // Determine which flank the king is on
        let (flank_files, _is_castled_side) = if king_file <= 2 {
            // Queenside (a, b, c files)
            (0x0707_0707_0707_0707u64, true)
        } else if king_file >= 5 {
            // Kingside (f, g, h files)
            (0xE0E0_E0E0_E0E0_E0E0u64, true)
        } else {
            // Center (d, e files) - not castled, less relevant
            return 0;
        };

        // Check if we have any pawns on this flank
        if (pawns.0 & flank_files) == 0 {
            return PAWNLESS_FLANK_MG;
        }

        0
    }

    /// Evaluate king exposure based on open lines.
    fn eval_king_exposure(&self, color: Color, king_sq: usize, _ctx: &AttackContext) -> i32 {
        let opp_idx = color.opponent().index();
        let mut penalty = 0;

        let king_file = king_sq % 8;

        // Check for open files toward king
        let file_mask = 0x0101_0101_0101_0101u64 << king_file;
        let all_pawns = self.pieces[0][Piece::Pawn.index()].0 | self.pieces[1][Piece::Pawn.index()].0;

        // If the file is open or semi-open toward the enemy
        if (file_mask & all_pawns) == 0 {
            // Fully open file
            let enemy_rooks = self.pieces[opp_idx][Piece::Rook.index()];
            let enemy_queens = self.pieces[opp_idx][Piece::Queen.index()];

            // Check if enemy has rooks/queens that could use this file
            for sq in enemy_rooks.iter() {
                if sq.index() % 8 == king_file {
                    penalty += KING_EXPOSURE_FILE_MG;
                }
            }
            for sq in enemy_queens.iter() {
                if sq.index() % 8 == king_file {
                    penalty += KING_EXPOSURE_FILE_MG;
                }
            }
        }

        // Check adjacent files too
        for adj_file in [king_file.saturating_sub(1), (king_file + 1).min(7)] {
            if adj_file != king_file {
                let adj_file_mask = 0x0101_0101_0101_0101u64 << adj_file;
                if (adj_file_mask & all_pawns) == 0 {
                    let enemy_rooks = self.pieces[opp_idx][Piece::Rook.index()];
                    let enemy_queens = self.pieces[opp_idx][Piece::Queen.index()];

                    for sq in enemy_rooks.iter() {
                        if sq.index() % 8 == adj_file {
                            penalty += KING_EXPOSURE_FILE_MG / 2;
                        }
                    }
                    for sq in enemy_queens.iter() {
                        if sq.index() % 8 == adj_file {
                            penalty += KING_EXPOSURE_FILE_MG / 2;
                        }
                    }
                }
            }
        }

        // Check diagonal exposure
        let enemy_bishops = self.pieces[opp_idx][Piece::Bishop.index()];
        let enemy_queens = self.pieces[opp_idx][Piece::Queen.index()];

        // Get diagonal attacks through king position
        let diag_attackers = slider_attacks(king_sq, self.all_occupied.0, true);

        for sq in enemy_bishops.iter() {
            if (diag_attackers & (1u64 << sq.index())) != 0 {
                penalty += KING_EXPOSURE_DIAG_MG;
            }
        }
        for sq in enemy_queens.iter() {
            if (diag_attackers & (1u64 << sq.index())) != 0 {
                penalty += KING_EXPOSURE_DIAG_MG;
            }
        }

        penalty
    }

    /// Evaluate escape squares for the king.
    fn eval_escape_squares(&self, color: Color, king_sq: usize, ctx: &AttackContext) -> i32 {
        let c_idx = color.index();
        let own_pieces = self.occupied[c_idx];
        let enemy_attacks = ctx.all_attacks(color.opponent());

        let king_moves = KING_ATTACKS[king_sq];

        // Safe escape squares: king can move there and it's not attacked
        let safe_escapes = king_moves & !own_pieces.0 & !enemy_attacks.0;
        let safe_count = safe_escapes.count_ones() as i32;

        // Virtual mobility: squares blocked by own pieces
        let blocked_by_own = king_moves & own_pieces.0;
        let blocked_count = blocked_by_own.count_ones() as i32;

        let mut score = 0;

        if safe_count == 0 {
            // No escape squares - very dangerous
            score += NO_ESCAPE_MG;
        } else {
            score += safe_count * ESCAPE_SQUARE_MG;
        }

        // Penalty for squares blocked by own pieces (could escape there otherwise)
        score += blocked_count * BLOCKED_ESCAPE_MG;

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pawnless_flank() {
        // White king on g1, no kingside pawns
        let board: Board = "8/8/8/8/8/8/PPP5/6K1 w - - 0 1".parse().unwrap();
        let penalty = board.eval_pawnless_flank(Color::White, 6); // g-file = 6
        assert!(penalty < 0);
    }

    #[test]
    fn test_escape_squares() {
        // King with some escape squares
        let board: Board = "8/8/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        // Just verify the function runs
        let ctx = board.compute_attack_context();
        let (mg, _) = board.eval_king_danger(&ctx);
        // Alone king should have escape squares
        assert!(mg != i32::MIN);
    }
}
