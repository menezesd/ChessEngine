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
mod tests {
    use super::*;

    #[test]
    fn test_kk_is_draw() {
        // Just two kings
        let board: Board = "8/8/8/4k3/8/8/4K3/8 w - - 0 1".parse().unwrap();
        assert_eq!(board.get_draw_multiplier(Color::White), DRAW_CERTAIN);
    }

    #[test]
    fn test_kn_vs_k_is_draw() {
        // King and knight vs lone king
        let board: Board = "8/8/8/4k3/8/8/4K3/4N3 w - - 0 1".parse().unwrap();
        assert_eq!(board.get_draw_multiplier(Color::White), DRAW_CERTAIN);
    }

    #[test]
    fn test_kb_vs_k_is_draw() {
        // King and bishop vs lone king
        let board: Board = "8/8/8/4k3/8/8/4K3/4B3 w - - 0 1".parse().unwrap();
        assert_eq!(board.get_draw_multiplier(Color::White), DRAW_CERTAIN);
    }

    #[test]
    fn test_knn_vs_k_is_drawish() {
        // Two knights vs lone king - theoretically drawn but keep signal for blunder mates
        let board: Board = "8/8/8/4k3/8/8/4K3/3NN3 w - - 0 1".parse().unwrap();
        assert_eq!(board.get_draw_multiplier(Color::White), DRAW_LIKELY);
    }

    #[test]
    fn test_kr_vs_km_is_drawish() {
        // Rook vs minor piece - usually draw
        let board: Board = "8/8/8/4k3/8/4n3/4K3/4R3 w - - 0 1".parse().unwrap();
        assert_eq!(board.get_draw_multiplier(Color::White), DRAW_LIKELY);
    }

    #[test]
    fn test_normal_position_no_scaling() {
        // Normal position with pawns
        let board = Board::new();
        assert_eq!(board.get_draw_multiplier(Color::White), NO_DRAW_SCALING);
    }

    #[test]
    fn test_kq_vs_k_is_winning() {
        // Queen vs lone king is winning
        let board: Board = "8/8/8/4k3/8/8/4K3/4Q3 w - - 0 1".parse().unwrap();
        // Strong side has major piece, should not be draw
        assert_eq!(board.get_draw_multiplier(Color::White), NO_DRAW_SCALING);
    }
}
