//! Drawish endgame detection.
//!
//! Detects positions that are likely draws and scales the evaluation accordingly.

use crate::board::state::Board;
use crate::board::types::{Color, Piece};

/// Multiplier for certain draws (KK, KNK, KBK)
const DRAW_CERTAIN: i32 = 0;

/// Multiplier for drawish positions (1/4 of normal evaluation)
const DRAW_LIKELY: i32 = 16;

/// Multiplier for normal positions (no draw scaling)
const NO_DRAW_SCALING: i32 = 64;

impl Board {
    /// Get draw multiplier for endgame evaluation.
    /// Returns 0-64, where 0 = certain draw, 64 = no draw scaling.
    #[must_use]
    pub fn get_draw_multiplier(&self, strong: Color) -> i32 {
        let weak = strong.opponent();

        // Count pieces
        let sp = self.pieces_of(strong, Piece::Pawn).popcount();
        let sn = self.pieces_of(strong, Piece::Knight).popcount();
        let sb = self.pieces_of(strong, Piece::Bishop).popcount();
        let sr = self.pieces_of(strong, Piece::Rook).popcount();
        let sq = self.pieces_of(strong, Piece::Queen).popcount();

        let wp = self.pieces_of(weak, Piece::Pawn).popcount();
        let wn = self.pieces_of(weak, Piece::Knight).popcount();
        let wb = self.pieces_of(weak, Piece::Bishop).popcount();
        let wr = self.pieces_of(weak, Piece::Rook).popcount();
        let wq = self.pieces_of(weak, Piece::Queen).popcount();

        let s_minor = sn + sb;
        let s_major = sr + sq;
        let w_minor = wn + wb;
        let w_major = wr + wq;

        // Strong side has no pawns
        if sp == 0 {
            // KK, KNK, KBK, KNNK - certain draws
            if s_major == 0 && s_minor <= 1 {
                return DRAW_CERTAIN;
            }
            // KNNK (two knights vs lone king) - theoretically drawn but keep small signal
            // to help engine make progress toward positions where opponent might blunder
            if s_major == 0 && sn == 2 && sb == 0 && w_minor == 0 && w_major == 0 && wp == 0 {
                return DRAW_LIKELY;
            }
        }

        // No pawns on either side - various drawn endings
        if sp == 0 && wp == 0 {
            // KR vs KM - usually draw
            if sr == 1 && sq == 0 && s_minor == 0 && wr == 0 && wq == 0 && w_minor == 1 {
                return DRAW_LIKELY;
            }
            // KRM vs KR - usually draw
            if sr == 1 && sq == 0 && s_minor == 1 && wr == 1 && wq == 0 && w_minor == 0 {
                return DRAW_LIKELY;
            }
            // KQM vs KQ - usually draw
            if sq == 1 && sr == 0 && s_minor == 1 && wq == 1 && wr == 0 && w_minor == 0 {
                return DRAW_LIKELY;
            }
            // Equal rooks/queens with no minors
            if sr == wr && sq == wq && s_minor == 0 && w_minor == 0 {
                return DRAW_LIKELY;
            }
            // Equal minors only
            if s_major == 0 && w_major == 0 && s_minor == w_minor {
                return DRAW_LIKELY;
            }
        }

        // Two minors vs one minor (drawish)
        if sp == 0 && s_major == 0 && s_minor == 2 && w_major == 0 && w_minor == 1 && wp == 0 {
            // Exception: two bishops vs knight can be winning
            if !(sb == 2 && wn == 1) {
                return DRAW_LIKELY;
            }
        }

        NO_DRAW_SCALING
    }
}

#[cfg(test)]
mod tests;
