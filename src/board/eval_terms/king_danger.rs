//! King danger refinements.
//!
//! Implements:
//! - Pawnless flank attack (extra danger when shield pawns missing)
//! - King exposure score (based on slider x-rays)
//! - Escape square count (king mobility in danger)
//! - Virtual mobility (squares king could escape to if not blocked by own pieces)

use crate::board::attack_tables::{slider_attacks, KING_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

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

        for color in Color::BOTH {
            let sign = color.sign();
            let (color_mg, color_eg) = self.eval_king_danger_for_color(color, ctx);
            mg += sign * color_mg;
            eg += sign * color_eg;
        }

        (mg, eg)
    }

    fn eval_king_danger_for_color(&self, color: Color, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let eg = 0; // Most king danger terms are MG only

        let king_sq = self.king_square_index(color);
        let king_file = king_sq % 8;

        // Pawnless flank attack
        mg += self.eval_pawnless_flank(color, king_file);

        // King exposure (open lines toward king)
        mg += self.eval_king_exposure(color, king_sq);

        // Escape squares
        mg += self.eval_escape_squares(color, king_sq, ctx);

        (mg, eg)
    }

    /// Evaluate pawnless flank penalty.
    /// Extra danger when the side has no pawns on the flank where king resides.
    fn eval_pawnless_flank(&self, color: Color, king_file: usize) -> i32 {
        let pawns = self.pieces_of(color, Piece::Pawn);

        // Determine which flank the king is on
        let flank_files = if king_file <= 2 {
            // Queenside (a, b, c files)
            Bitboard::QUEENSIDE_FILES.0
        } else if king_file >= 5 {
            // Kingside (f, g, h files)
            Bitboard::KINGSIDE_FILES.0
        } else {
            // Center (d, e files) - not castled, less relevant
            return 0;
        };

        // Check if we have any pawns on this flank
        if pawns.is_disjoint(Bitboard(flank_files)) {
            return PAWNLESS_FLANK_MG;
        }

        0
    }

    /// Evaluate king exposure based on open lines.
    fn eval_king_exposure(&self, color: Color, king_sq: usize) -> i32 {
        let mut penalty = 0;

        let king_file = king_sq % 8;

        // Check for open files toward king
        let file_mask = Bitboard::FILE_A.0 << king_file;
        let all_pawns = self.pieces_of(Color::White, Piece::Pawn).0
            | self.pieces_of(Color::Black, Piece::Pawn).0;

        // If the file is open or semi-open toward the enemy
        if (file_mask & all_pawns) == 0 {
            // Fully open file
            penalty += self.enemy_heavy_pieces_on_file(color, king_file) * KING_EXPOSURE_FILE_MG;
        }

        // Check adjacent files too
        for adj_file in [king_file.saturating_sub(1), (king_file + 1).min(7)] {
            if adj_file != king_file {
                let adj_file_mask = Bitboard::FILE_A.0 << adj_file;
                if (adj_file_mask & all_pawns) == 0 {
                    penalty += self.enemy_heavy_pieces_on_file(color, adj_file)
                        * (KING_EXPOSURE_FILE_MG / 2);
                }
            }
        }

        // Check diagonal exposure
        let enemy_bishops = self.opponent_pieces(color, Piece::Bishop);
        let enemy_queens = self.opponent_pieces(color, Piece::Queen);

        // Get diagonal attacks through king position
        let diag_attackers = slider_attacks(king_sq, self.all_occupied.0, true);

        let diag_bb = Bitboard(diag_attackers);
        for sq in enemy_bishops.iter() {
            if diag_bb.has_bit(sq.index()) {
                penalty += KING_EXPOSURE_DIAG_MG;
            }
        }
        for sq in enemy_queens.iter() {
            if diag_bb.has_bit(sq.index()) {
                penalty += KING_EXPOSURE_DIAG_MG;
            }
        }

        penalty
    }

    fn enemy_heavy_pieces_on_file(&self, color: Color, file: usize) -> i32 {
        let enemy_rooks = self.opponent_pieces(color, Piece::Rook);
        let enemy_queens = self.opponent_pieces(color, Piece::Queen);

        enemy_rooks
            .iter()
            .chain(enemy_queens.iter())
            .filter(|sq| sq.file() == file)
            .count() as i32
    }

    /// Evaluate escape squares for the king.
    fn eval_escape_squares(&self, color: Color, king_sq: usize, ctx: &AttackContext) -> i32 {
        let own_pieces = self.occupied_by(color);
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
mod tests;
