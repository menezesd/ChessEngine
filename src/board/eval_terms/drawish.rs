//! Drawish endgame detection.
//!
//! Detects positions that are likely draws and scales the evaluation accordingly.

use crate::board::state::Board;
use crate::board::types::{Color, Piece};

impl Board {
    /// Get draw multiplier for endgame evaluation.
    /// Returns 0-64, where 0 = certain draw, 64 = no draw scaling.
    #[must_use]
    pub fn get_draw_multiplier(&self, strong: Color) -> i32 {
        let strong_idx = strong.index();
        let weak_idx = 1 - strong_idx;

        // Count pieces
        let sp = self.pieces[strong_idx][Piece::Pawn.index()].popcount();
        let sn = self.pieces[strong_idx][Piece::Knight.index()].popcount();
        let sb = self.pieces[strong_idx][Piece::Bishop.index()].popcount();
        let sr = self.pieces[strong_idx][Piece::Rook.index()].popcount();
        let sq = self.pieces[strong_idx][Piece::Queen.index()].popcount();

        let wp = self.pieces[weak_idx][Piece::Pawn.index()].popcount();
        let wn = self.pieces[weak_idx][Piece::Knight.index()].popcount();
        let wb = self.pieces[weak_idx][Piece::Bishop.index()].popcount();
        let wr = self.pieces[weak_idx][Piece::Rook.index()].popcount();
        let wq = self.pieces[weak_idx][Piece::Queen.index()].popcount();

        let s_minor = sn + sb;
        let s_major = sr + sq;
        let w_minor = wn + wb;
        let w_major = wr + wq;

        // Strong side has no pawns
        if sp == 0 {
            // KK, KNK, KBK, KNNK - certain draws
            if s_major == 0 && s_minor <= 1 {
                return 0;
            }
            // KNNK (two knights vs lone king)
            if s_major == 0 && sn == 2 && sb == 0 && w_minor == 0 && w_major == 0 && wp == 0 {
                return 0;
            }
        }

        // No pawns on either side - various drawn endings
        if sp == 0 && wp == 0 {
            // KR vs KM - usually draw
            if sr == 1 && sq == 0 && s_minor == 0 && wr == 0 && wq == 0 && w_minor == 1 {
                return 16;
            }
            // KRM vs KR - usually draw
            if sr == 1 && sq == 0 && s_minor == 1 && wr == 1 && wq == 0 && w_minor == 0 {
                return 16;
            }
            // KQM vs KQ - usually draw
            if sq == 1 && sr == 0 && s_minor == 1 && wq == 1 && wr == 0 && w_minor == 0 {
                return 16;
            }
            // Equal rooks/queens with no minors
            if sr == wr && sq == wq && s_minor == 0 && w_minor == 0 {
                return 16;
            }
            // Equal minors only
            if s_major == 0 && w_major == 0 && s_minor == w_minor {
                return 16;
            }
        }

        // Two minors vs one minor (drawish)
        if sp == 0 && s_major == 0 && s_minor == 2 && w_major == 0 && w_minor == 1 && wp == 0 {
            // Exception: two bishops vs knight can be winning
            if !(sb == 2 && wn == 1) {
                return 16;
            }
        }

        64 // No draw scaling
    }
}
