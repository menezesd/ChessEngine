//! Incremental evaluation update helpers.
//!
//! Provides utilities for maintaining incremental evaluation scores
//! during make/unmake operations.

use super::pst::{MATERIAL_EG, MATERIAL_MG, PHASE_WEIGHTS, PST_EG, PST_MG};
use super::state::Board;
use super::types::Piece;

/// Calculate PST square index for a given color.
/// White uses the square index directly, Black mirrors vertically.
#[inline]
#[must_use]
pub fn pst_square(sq_idx: usize, is_white: bool) -> usize {
    if is_white {
        sq_idx
    } else {
        sq_idx ^ 0b11_1000
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
}

#[cfg(test)]
mod tests;
