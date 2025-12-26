//! Incremental evaluation update helpers.
//!
//! Provides utilities for maintaining incremental evaluation scores
//! during make/unmake operations.

use super::pst::{MATERIAL_EG, MATERIAL_MG, PHASE_WEIGHTS, PST_EG, PST_MG};
use super::state::Board;
use super::types::{Color, Piece};

/// Calculate PST square index for a given color.
/// White uses the square index directly, Black mirrors vertically.
#[inline]
#[must_use]
pub fn pst_square(sq_idx: usize, is_white: bool) -> usize {
    if is_white {
        sq_idx
    } else {
        sq_idx ^ 56
    }
}

/// Incremental evaluation state.
///
/// This struct encapsulates the three arrays that make up the
/// incremental evaluation: middlegame scores, endgame scores,
/// and game phase values for each color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvalState {
    /// Middlegame scores [white, black]
    pub mg: [i32; 2],
    /// Endgame scores [white, black]
    pub eg: [i32; 2],
    /// Game phase contributions [white, black]
    pub phase: [i32; 2],
}

impl EvalState {
    /// Create a new zeroed eval state.
    #[must_use]
    pub const fn new() -> Self {
        EvalState {
            mg: [0, 0],
            eg: [0, 0],
            phase: [0, 0],
        }
    }

    /// Add a piece to the evaluation.
    #[inline]
    pub fn add_piece(&mut self, color_idx: usize, piece: Piece, sq_idx: usize, is_white: bool) {
        let p_idx = piece.index();
        let pst_sq = pst_square(sq_idx, is_white);

        self.mg[color_idx] += MATERIAL_MG[p_idx] + PST_MG[p_idx][pst_sq];
        self.eg[color_idx] += MATERIAL_EG[p_idx] + PST_EG[p_idx][pst_sq];
        self.phase[color_idx] += PHASE_WEIGHTS[p_idx];
    }

    /// Remove a piece from the evaluation.
    #[inline]
    pub fn remove_piece(&mut self, color_idx: usize, piece: Piece, sq_idx: usize, is_white: bool) {
        let p_idx = piece.index();
        let pst_sq = pst_square(sq_idx, is_white);

        self.mg[color_idx] -= MATERIAL_MG[p_idx] + PST_MG[p_idx][pst_sq];
        self.eg[color_idx] -= MATERIAL_EG[p_idx] + PST_EG[p_idx][pst_sq];
        self.phase[color_idx] -= PHASE_WEIGHTS[p_idx];
    }

    /// Move a piece from one square to another.
    #[inline]
    pub fn move_piece(
        &mut self,
        color_idx: usize,
        piece: Piece,
        from_idx: usize,
        to_idx: usize,
        is_white: bool,
    ) {
        let p_idx = piece.index();
        let from_pst = pst_square(from_idx, is_white);
        let to_pst = pst_square(to_idx, is_white);

        // Remove from old square
        self.mg[color_idx] -= MATERIAL_MG[p_idx] + PST_MG[p_idx][from_pst];
        self.eg[color_idx] -= MATERIAL_EG[p_idx] + PST_EG[p_idx][from_pst];
        self.phase[color_idx] -= PHASE_WEIGHTS[p_idx];

        // Add to new square
        self.mg[color_idx] += MATERIAL_MG[p_idx] + PST_MG[p_idx][to_pst];
        self.eg[color_idx] += MATERIAL_EG[p_idx] + PST_EG[p_idx][to_pst];
        self.phase[color_idx] += PHASE_WEIGHTS[p_idx];
    }
}

impl Default for EvalState {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    /// Get a snapshot of the current eval state.
    #[must_use]
    pub fn eval_state(&self) -> EvalState {
        EvalState {
            mg: self.eval_mg,
            eg: self.eval_eg,
            phase: self.game_phase,
        }
    }

    /// Set the eval state from a snapshot.
    pub fn set_eval_state(&mut self, state: EvalState) {
        self.eval_mg = state.mg;
        self.eval_eg = state.eg;
        self.game_phase = state.phase;
    }

    /// Add a piece to incremental evaluation.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn eval_add_piece(&mut self, color: Color, piece: Piece, sq_idx: usize) {
        let c_idx = color.index();
        let is_white = color == Color::White;
        let p_idx = piece.index();
        let pst_sq = pst_square(sq_idx, is_white);

        self.eval_mg[c_idx] += MATERIAL_MG[p_idx] + PST_MG[p_idx][pst_sq];
        self.eval_eg[c_idx] += MATERIAL_EG[p_idx] + PST_EG[p_idx][pst_sq];
        self.game_phase[c_idx] += PHASE_WEIGHTS[p_idx];
    }

    /// Remove a piece from incremental evaluation.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn eval_remove_piece(&mut self, color: Color, piece: Piece, sq_idx: usize) {
        let c_idx = color.index();
        let is_white = color == Color::White;
        let p_idx = piece.index();
        let pst_sq = pst_square(sq_idx, is_white);

        self.eval_mg[c_idx] -= MATERIAL_MG[p_idx] + PST_MG[p_idx][pst_sq];
        self.eval_eg[c_idx] -= MATERIAL_EG[p_idx] + PST_EG[p_idx][pst_sq];
        self.game_phase[c_idx] -= PHASE_WEIGHTS[p_idx];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pst_square_white() {
        // White's a1 (index 0) stays 0
        assert_eq!(pst_square(0, true), 0);
        // White's h8 (index 63) stays 63
        assert_eq!(pst_square(63, true), 63);
    }

    #[test]
    fn test_pst_square_black() {
        // Black's a1 (index 0) becomes a8 (index 56)
        assert_eq!(pst_square(0, false), 56);
        // Black's h8 (index 63) becomes h1 (index 7)
        assert_eq!(pst_square(63, false), 7);
    }

    #[test]
    fn test_eval_state_add_remove() {
        let mut state = EvalState::new();

        // Add a white pawn at e2 (index 12)
        state.add_piece(0, Piece::Pawn, 12, true);
        assert!(state.mg[0] > 0);
        assert!(state.eg[0] > 0);

        // Remove it
        state.remove_piece(0, Piece::Pawn, 12, true);
        assert_eq!(state.mg[0], 0);
        assert_eq!(state.eg[0], 0);
    }
}
