/// Total phase value (sum of all pieces' phase weights at game start)
const PHASE_TOTAL: i32 = 24;

/// Accumulated evaluation score with middlegame and endgame components.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct EvalScore {
    pub(super) mg: i32,
    pub(super) eg: i32,
}

impl EvalScore {
    /// Create a new score from mg/eg tuple.
    #[inline]
    pub(super) const fn new(mg: i32, eg: i32) -> Self {
        EvalScore { mg, eg }
    }

    /// Create a score where mg and eg are the same (e.g., for bonuses).
    #[inline]
    pub(super) const fn both(value: i32) -> Self {
        EvalScore {
            mg: value,
            eg: value,
        }
    }

    /// Create a score with only middlegame component.
    #[inline]
    pub(super) const fn mg_only(mg: i32) -> Self {
        EvalScore { mg, eg: 0 }
    }
}

impl std::ops::Add for EvalScore {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        EvalScore {
            mg: self.mg + other.mg,
            eg: self.eg + other.eg,
        }
    }
}

impl std::ops::AddAssign for EvalScore {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.mg += other.mg;
        self.eg += other.eg;
    }
}

impl From<(i32, i32)> for EvalScore {
    #[inline]
    fn from((mg, eg): (i32, i32)) -> Self {
        EvalScore { mg, eg }
    }
}

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
    pub(super) fn from_game_phase(white_phase: i32, black_phase: i32) -> Self {
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
    pub(super) fn taper(&self, mg_score: i32, eg_score: i32) -> i32 {
        (mg_score * self.midphase + self.endgame_mult * eg_score * self.endphase) / PHASE_TOTAL
    }

    #[inline]
    pub(super) fn taper_pair(&self, (mg_score, eg_score): (i32, i32)) -> i32 {
        self.taper(mg_score, eg_score)
    }
}
