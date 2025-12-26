//! Position evaluation using tapered eval.
//!
//! Uses incremental evaluation (`eval_mg`, `eval_eg`, `game_phase`) stored in Board.
//! Implements tapered evaluation with advanced evaluation terms including:
//! - Material and piece-square tables (incremental)
//! - Bishop pair bonus
//! - Mobility
//! - Pawn structure (passed, doubled, isolated, backward)
//! - King safety (attack units, pawn shield)
//! - Rook activity (open files, 7th rank)
//! - Hanging pieces
//! - Drawish endgame detection

use super::{Board, Color, Piece};

/// Bishop pair bonus in centipawns
const BISHOP_PAIR_BONUS: i32 = 37;

/// Tempo bonus (side to move advantage)
const TEMPO_BONUS: i32 = 11;

/// Total phase value (sum of all pieces' phase weights at game start)
const PHASE_TOTAL: i32 = 24;

/// Phase factors for tapered evaluation.
///
/// Encapsulates the middlegame/endgame interpolation weights.
#[derive(Debug, Clone, Copy)]
struct PhaseFactors {
    /// Weight for middlegame evaluation (0-24)
    midphase: i32,
    /// Weight for endgame evaluation (0-24)
    endphase: i32,
    /// Multiplier for endgame when one side has only pawns (1 or 2)
    endgame_mult: i32,
}

impl PhaseFactors {
    /// Compute phase factors from game phase values.
    #[inline]
    fn from_game_phase(white_phase: i32, black_phase: i32) -> Self {
        let midphase = (white_phase + black_phase).min(PHASE_TOTAL);
        let endphase = PHASE_TOTAL - midphase;
        // Double endgame weight when one side has no non-pawn pieces
        let endgame_mult = if white_phase.min(black_phase) == 0 {
            2
        } else {
            1
        };
        PhaseFactors {
            midphase,
            endphase,
            endgame_mult,
        }
    }

    /// Apply tapered evaluation to middlegame and endgame scores.
    #[inline]
    fn taper(&self, mg_score: i32, eg_score: i32) -> i32 {
        (mg_score * self.midphase + self.endgame_mult * eg_score * self.endphase) / PHASE_TOTAL
    }
}

impl Board {
    /// Evaluate the position from the side-to-move's perspective.
    ///
    /// Uses tapered evaluation to interpolate between middlegame and endgame scores
    /// based on the current game phase. Includes all evaluation terms.
    #[must_use]
    pub fn evaluate(&self) -> i32 {
        let phase = PhaseFactors::from_game_phase(self.game_phase[0], self.game_phase[1]);

        // Base incremental scores (material + PST)
        let base_mg = self.eval_mg[0] - self.eval_mg[1];
        let base_eg = self.eval_eg[0] - self.eval_eg[1];

        // Bishop pair bonus
        let white_bishops = self.pieces[0][Piece::Bishop.index()].popcount();
        let black_bishops = self.pieces[1][Piece::Bishop.index()].popcount();
        let bishop_bonus =
            BISHOP_PAIR_BONUS * ((white_bishops / 2) as i32 - (black_bishops / 2) as i32);

        // Compute attack context once for all evaluation terms
        let ctx = self.compute_attack_context();

        // Advanced evaluation terms (all from white's perspective)
        let (mob_mg, mob_eg) = self.eval_mobility_with_context(&ctx);
        let (pawn_mg, pawn_eg) = self.eval_pawn_structure();
        let (king_mg, king_eg) = self.eval_king_safety_with_context(&ctx);
        let (shield_mg, shield_eg) = self.eval_king_shield();
        let (rook_mg, rook_eg) = self.eval_rooks();

        // Combined evaluation for passed pawns and hanging pieces (shares attack computation)
        let (pass_mg, pass_eg, hanging) = self.eval_attacks_dependent_with_context(&ctx);

        // Combine all middlegame terms
        let total_mg = base_mg
            + bishop_bonus
            + mob_mg
            + pawn_mg
            + pass_mg
            + king_mg
            + shield_mg
            + rook_mg
            + hanging;

        // Combine all endgame terms
        let total_eg = base_eg
            + bishop_bonus
            + mob_eg
            + pawn_eg
            + pass_eg
            + king_eg
            + shield_eg
            + rook_eg
            + hanging;

        // Tapered evaluation
        let mut score = phase.taper(total_mg, total_eg) + TEMPO_BONUS;

        // Apply draw multiplier in endgames
        if phase.endphase > 12 {
            let strong = if score > 0 {
                Color::White
            } else {
                Color::Black
            };
            let mul = self.get_draw_multiplier(strong);
            score = score * mul / 64;
        }

        // Return from side-to-move perspective
        if self.white_to_move {
            score
        } else {
            -score
        }
    }

    /// Simple/fast evaluation for quiescence or pruning decisions.
    /// Only uses incremental material + PST + bishop pair.
    #[must_use]
    pub fn evaluate_simple(&self) -> i32 {
        let c_idx = usize::from(!self.white_to_move);
        let opp_idx = 1 - c_idx;

        let phase = PhaseFactors::from_game_phase(self.game_phase[0], self.game_phase[1]);

        let mideval = self.eval_mg[c_idx] - self.eval_mg[opp_idx];
        let endeval = self.eval_eg[c_idx] - self.eval_eg[opp_idx];

        let our_bishops = self.pieces[c_idx][Piece::Bishop.index()].popcount();
        let opp_bishops = self.pieces[opp_idx][Piece::Bishop.index()].popcount();
        let bishop_bonus =
            BISHOP_PAIR_BONUS * ((our_bishops / 2) as i32 - (opp_bishops / 2) as i32);

        phase.taper(mideval, endeval) + bishop_bonus + TEMPO_BONUS
    }
}
