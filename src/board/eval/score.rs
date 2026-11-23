/// Total phase value (sum of all pieces' phase weights at game start)
const PHASE_TOTAL: i32 = 24;

/// Phase factors for tapered evaluation.
///
/// Encapsulates the middlegame/endgame interpolation weights.
#[derive(Debug, Clone, Copy)]
pub(super) struct PhaseFactors {
    /// Weight for middlegame evaluation (0-24)
    pub(super) midphase: i32,
    /// Weight for endgame evaluation (0-24)
    pub(super) endphase: i32,
    /// Multiplier for endgame when one side has only pawns (1 or 2)
    endgame_mult: i32,
}

impl PhaseFactors {
    /// Compute phase factors from game phase values.
    #[inline]
    pub(super) fn from_game_phase([white_phase, black_phase]: [i32; 2]) -> Self {
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
    pub(super) fn taper(&self, (mg_score, eg_score): (i32, i32)) -> i32 {
        (mg_score * self.midphase + self.endgame_mult * eg_score * self.endphase) / PHASE_TOTAL
    }

    #[inline]
    pub(super) fn taper_total<const N: usize>(&self, terms: [(i32, i32); N]) -> i32 {
        let scores = terms
            .into_iter()
            .fold((0, 0), |(mg, eg), term| (mg + term.0, eg + term.1));
        self.taper(scores)
    }
}
