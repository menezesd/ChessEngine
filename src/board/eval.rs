//! Position evaluation using tapered eval.
//!
//! Uses incremental evaluation (`eval_mg`, `eval_eg`, `game_phase`) stored in Board.
//! Implements tapered evaluation with advanced evaluation terms including:
//! - Material and piece-square tables (incremental)
//! - Bishop pair bonus
//! - Bishop vs knight imbalance (bishops better in open positions)
//! - Mobility
//! - Pawn structure (passed, doubled, isolated, backward)
//! - King safety (attack units, pawn shield)
//! - Rook activity (open files, 7th rank)
//! - Hanging pieces
//! - Drawish endgame detection

use super::{Board, Color, Piece};

/// Bishop pair bonus in centipawns (Texel tuned v2)
const BISHOP_PAIR_BONUS: i32 = 18;

/// Tempo bonus (side to move advantage) (Texel tuned v2)
const TEMPO_BONUS: i32 = 19;

/// Total phase value (sum of all pieces' phase weights at game start)
const PHASE_TOTAL: i32 = 24;

/// Bishop vs Knight imbalance bonus per pawn difference from 8.
/// Bishops are worth more in open positions (fewer pawns).
/// Formula: `bishop_bonus` = (8 - `total_pawns`) * `BISHOP_OPEN_BONUS` per bishop advantage
/// (Texel tuned v2)
const BISHOP_OPEN_BONUS: i32 = 12;

/// Accumulated evaluation score with middlegame and endgame components.
#[derive(Debug, Clone, Copy, Default)]
struct EvalScore {
    mg: i32,
    eg: i32,
}

impl EvalScore {
    /// Create a new score from mg/eg tuple.
    #[inline]
    const fn new(mg: i32, eg: i32) -> Self {
        EvalScore { mg, eg }
    }

    /// Create a score where mg and eg are the same (e.g., for bonuses).
    #[inline]
    const fn both(value: i32) -> Self {
        EvalScore {
            mg: value,
            eg: value,
        }
    }

    /// Create a score with only middlegame component.
    #[inline]
    const fn mg_only(mg: i32) -> Self {
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
        let white_bishops = self.pieces_of(Color::White, Piece::Bishop).popcount();
        let black_bishops = self.pieces_of(Color::Black, Piece::Bishop).popcount();
        let bishop_pair_bonus =
            BISHOP_PAIR_BONUS * ((white_bishops / 2) as i32 - (black_bishops / 2) as i32);

        // Bishop vs Knight imbalance: bishops better in open positions
        let white_knights = self.pieces_of(Color::White, Piece::Knight).popcount();
        let black_knights = self.pieces_of(Color::Black, Piece::Knight).popcount();
        let total_pawns = self.pieces_of(Color::White, Piece::Pawn).popcount()
            + self.pieces_of(Color::Black, Piece::Pawn).popcount();
        let openness = (16 - total_pawns as i32).max(0); // 0 when 16 pawns, 16 when 0 pawns

        // Net bishop advantage (bishops - knights for each side)
        let white_bishop_adv = white_bishops as i32 - white_knights as i32;
        let black_bishop_adv = black_bishops as i32 - black_knights as i32;
        let bishop_imbalance =
            (white_bishop_adv - black_bishop_adv) * openness * BISHOP_OPEN_BONUS / 8;

        let bishop_bonus = bishop_pair_bonus + bishop_imbalance;

        // Compute attack context once for all evaluation terms
        let ctx = self.compute_attack_context();

        // Accumulate all evaluation terms using EvalScore
        let mut total = EvalScore::new(base_mg, base_eg);
        total += EvalScore::both(bishop_bonus);

        // Advanced evaluation terms (all from white's perspective)
        total += self.eval_mobility_with_context(&ctx).into();
        total += self.eval_pawn_structure().into();
        total += self.eval_king_safety_with_context(&ctx).into();
        total += self.eval_king_shield().into();
        total += self.eval_rooks().into();
        total += self.eval_minor_pieces(&ctx).into();
        total += EvalScore::mg_only(self.eval_tropism());

        // Combined evaluation for passed pawns and hanging pieces (shares attack computation)
        let (pass_mg, pass_eg, hanging) = self.eval_attacks_dependent_with_context(&ctx);
        total += EvalScore::new(pass_mg, pass_eg);
        total += EvalScore::both(hanging);

        // Additional advanced evaluation terms
        total += self.eval_coordination(&ctx).into();
        total += self.eval_pawn_advanced().into();
        total += self.eval_weak_squares(&ctx).into();
        total += self.eval_king_danger(&ctx).into();
        total += self.eval_endgame_patterns().into();
        total += self.eval_space_control(&ctx).into();
        total += self.eval_threats_advanced(&ctx).into();
        total += self.eval_piece_quality(&ctx).into();
        total += self.eval_imbalances().into();
        total += self.eval_initiative(&ctx).into();

        // Tapered evaluation
        let mut score = phase.taper(total.mg, total.eg) + TEMPO_BONUS;

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
    /// Note: Bishop imbalance is only in full eval to keep simple eval fast.
    #[must_use]
    pub fn evaluate_simple(&self) -> i32 {
        let stm = self.side_to_move();
        let stm_idx = stm.index();
        let opp_idx = stm.opponent().index();

        let phase = PhaseFactors::from_game_phase(self.game_phase[0], self.game_phase[1]);

        let mideval = self.eval_mg[stm_idx] - self.eval_mg[opp_idx];
        let endeval = self.eval_eg[stm_idx] - self.eval_eg[opp_idx];

        // Bishop pair bonus only (imbalance is in full eval)
        let our_bishops = self.pieces_of(stm, Piece::Bishop).popcount();
        let opp_bishops = self.opponent_pieces(stm, Piece::Bishop).popcount();
        let bishop_bonus =
            BISHOP_PAIR_BONUS * ((our_bishops / 2) as i32 - (opp_bishops / 2) as i32);

        phase.taper(mideval, endeval) + bishop_bonus + TEMPO_BONUS
    }

    /// Compute active NNUE features for both perspectives.
    /// Returns (`white_features`, `black_features`) as vectors of feature indices.
    #[must_use]
    pub fn compute_nnue_features(&self) -> (Vec<usize>, Vec<usize>) {
        use super::nnue::network::feature_index;

        let mut white_features = Vec::with_capacity(32);
        let mut black_features = Vec::with_capacity(32);

        for color in Color::BOTH {
            let color_idx = color.index();
            for piece in Piece::ALL {
                let piece_idx = piece.index();
                for sq in self.pieces_of(color, piece).iter() {
                    let sq_idx = sq.as_index();
                    // White's perspective
                    white_features.push(feature_index(piece_idx, color_idx, sq_idx, 0));
                    // Black's perspective
                    black_features.push(feature_index(piece_idx, color_idx, sq_idx, 1));
                }
            }
        }

        (white_features, black_features)
    }

    /// Evaluate position using NNUE network.
    /// Returns score in centipawns from side-to-move perspective.
    #[must_use]
    pub fn evaluate_nnue(&self, network: &super::nnue::NnueNetwork) -> i32 {
        use super::nnue::NnueAccumulator;

        // Compute features from scratch (non-incremental)
        let (white_features, black_features) = self.compute_nnue_features();

        // Build accumulator
        let mut acc = NnueAccumulator::new(&network.feature_bias);
        acc.refresh(&white_features, &black_features, network);

        // Evaluate
        network.evaluate(&acc, self.white_to_move)
    }
}
