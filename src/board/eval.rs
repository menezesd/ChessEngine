//! Position evaluation using tapered eval.
//!
//! Uses incremental evaluation (`eval_mg`, `eval_eg`, `game_phase`) stored in Board.
//! Implements tapered evaluation with bishop pair bonus and tempo.

use super::{Board, Piece};

/// Bishop pair bonus in centipawns
const BISHOP_PAIR_BONUS: i32 = 37;

/// Tempo bonus (side to move advantage)
const TEMPO_BONUS: i32 = 11;

impl Board {
    /// Evaluate the position from the side-to-move's perspective.
    ///
    /// Uses tapered evaluation to interpolate between middlegame and endgame scores
    /// based on the current game phase. Includes bishop pair bonus and tempo.
    #[must_use]
    pub fn evaluate(&self) -> i32 {
        let c_idx = usize::from(!self.white_to_move);
        let opp_idx = 1 - c_idx;

        // Calculate game phase (capped at 24)
        let midphase = (self.game_phase[0] + self.game_phase[1]).min(24);
        let endphase = 24 - midphase;

        // Calculate score differences
        let mideval = self.eval_mg[c_idx] - self.eval_mg[opp_idx];
        let endeval = self.eval_eg[c_idx] - self.eval_eg[opp_idx];

        // Bishop pair bonus
        let our_bishops = (self.pieces[c_idx][Piece::Bishop.index()].0).count_ones();
        let opp_bishops = (self.pieces[opp_idx][Piece::Bishop.index()].0).count_ones();
        let bishop_bonus = BISHOP_PAIR_BONUS * ((our_bishops / 2) as i32 - (opp_bishops / 2) as i32);

        // Endgame multiplier when one side has no non-pawn pieces
        let endgame_mult = if self.game_phase[0].min(self.game_phase[1]) == 0 {
            2
        } else {
            1
        };

        // Tapered evaluation + bishop bonus + tempo
        (mideval * midphase + endgame_mult * endeval * endphase) / 24 + bishop_bonus + TEMPO_BONUS
    }
}
