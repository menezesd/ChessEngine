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

use super::attack_tables::{queen_attacks, slider_attacks, KNIGHT_ATTACKS};
use super::eval_terms::helpers::AttackContext;
use super::masks::KING_ZONE_EXTENDED;
use super::{Board, Color, Piece, Square};

mod score;

use score::{EvalScore, PhaseFactors};

/// Bishop pair bonus in centipawns (Texel tuned v2)
const BISHOP_PAIR_BONUS: i32 = 18;

/// Tempo bonus (side to move advantage) (Texel tuned v2)
const TEMPO_BONUS: i32 = 19;

/// Bishop vs Knight imbalance bonus per pawn difference from 8.
/// Bishops are worth more in open positions (fewer pawns).
/// Formula: `bishop_bonus` = (8 - `total_pawns`) * `BISHOP_OPEN_BONUS` per bishop advantage
/// (Texel tuned v2)
const BISHOP_OPEN_BONUS: i32 = 12;
const ENDGAME_DRAW_SCALING_PHASE: i32 = 12;
const DRAW_SCALING_DENOMINATOR: i32 = 64;

pub const HCE_FEATURE_NAMES: [&str; 21] = [
    "base",
    "bishop",
    "mobility",
    "pawn_structure",
    "king_safety",
    "king_shield",
    "rooks",
    "minor_pieces",
    "tropism",
    "passed_pawns",
    "hanging",
    "coordination",
    "pawn_advanced",
    "weak_squares",
    "king_danger",
    "endgame_patterns",
    "space_control",
    "threats_advanced",
    "piece_quality",
    "imbalances",
    "initiative",
];

const TUNED_HCE_WEIGHTS_PERMILLE: [i32; HCE_FEATURE_NAMES.len()] = [
    1078, 1052, 1091, 978, 1052, 1046, 1069, 1001, 1002, 1094, 1134, 972, 1019, 967, 1035, 1009,
    1029, 975, 1004, 1005, 1005,
];

#[derive(Debug, Clone)]
pub struct HceFeatureBreakdown {
    /// Individually tapered, untuned per-feature values in White's perspective.
    /// Their sum can differ slightly from `phase_score` because the engine
    /// tapers the aggregate middlegame/endgame score with one final rounding.
    pub values: [i32; HCE_FEATURE_NAMES.len()],
    /// Side-to-move tempo expressed in white's perspective.
    pub tempo: i32,
    /// Aggregate untuned HCE score before tempo and draw scaling.
    pub phase_score: i32,
    /// Untuned full HCE score from White's perspective.
    pub full_white: i32,
    /// Tuned full HCE score from White's perspective.
    pub tuned_full_white: i32,
}

impl Board {
    fn bishop_bonus(&self) -> i32 {
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

        bishop_pair_bonus + bishop_imbalance
    }

    fn advanced_eval_terms_with_pawn_structure(&self, pawn_structure: (i32, i32)) -> EvalScore {
        let ctx = self.compute_attack_context();
        let mut total = EvalScore::default();

        // Advanced evaluation terms (all from white's perspective)
        total += self.eval_mobility_with_context(&ctx).into();
        total += pawn_structure.into();
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
        total += self.eval_threats_advanced().into();
        total += self.eval_piece_quality(&ctx).into();
        total += self.eval_imbalances().into();
        total += self.eval_initiative(&ctx).into();

        total
    }

    #[must_use]
    fn hce_feature_breakdown_with_pawn_structure(
        &self,
        pawn_structure: (i32, i32),
    ) -> HceFeatureBreakdown {
        let phase = PhaseFactors::from_game_phase(self.game_phase[0], self.game_phase[1]);
        let ctx = self.compute_attack_context();
        let mut term_scores = [(0, 0); HCE_FEATURE_NAMES.len()];
        let mut values = [0; HCE_FEATURE_NAMES.len()];

        let base_mg = self.eval_mg[0] - self.eval_mg[1];
        let base_eg = self.eval_eg[0] - self.eval_eg[1];
        let bishop_bonus = self.bishop_bonus();
        term_scores[0] = (base_mg, base_eg);
        term_scores[1] = (bishop_bonus, bishop_bonus);
        term_scores[2] = self.eval_mobility_with_context(&ctx);
        term_scores[3] = pawn_structure;
        term_scores[4] = self.eval_king_safety_with_context(&ctx);
        term_scores[5] = self.eval_king_shield();
        term_scores[6] = self.eval_rooks();
        term_scores[7] = self.eval_minor_pieces(&ctx);
        term_scores[8] = (self.eval_tropism(), 0);

        let (pass_mg, pass_eg, hanging) = self.eval_attacks_dependent_with_context(&ctx);
        term_scores[9] = (pass_mg, pass_eg);
        term_scores[10] = (hanging, hanging);
        term_scores[11] = self.eval_coordination(&ctx);
        term_scores[12] = self.eval_pawn_advanced();
        term_scores[13] = self.eval_weak_squares(&ctx);
        term_scores[14] = self.eval_king_danger(&ctx);
        term_scores[15] = self.eval_endgame_patterns();
        term_scores[16] = self.eval_space_control(&ctx);
        term_scores[17] = self.eval_threats_advanced();
        term_scores[18] = self.eval_piece_quality(&ctx);
        term_scores[19] = self.eval_imbalances();
        term_scores[20] = self.eval_initiative(&ctx);

        for (value, term) in values.iter_mut().zip(term_scores) {
            *value = phase.taper_pair(term);
        }
        let (total_mg, total_eg) = term_scores
            .into_iter()
            .fold((0, 0), |total, term| (total.0 + term.0, total.1 + term.1));
        let phase_score = phase.taper(total_mg, total_eg);
        let tempo = if self.white_to_move {
            TEMPO_BONUS
        } else {
            -TEMPO_BONUS
        };
        let mut full_white = phase_score + tempo;

        if phase.endphase > ENDGAME_DRAW_SCALING_PHASE {
            let strong = if full_white > 0 {
                Color::White
            } else {
                Color::Black
            };
            let mul = self.get_draw_multiplier(strong);
            full_white = full_white * mul / DRAW_SCALING_DENOMINATOR;
        }

        let weighted = values
            .iter()
            .zip(TUNED_HCE_WEIGHTS_PERMILLE)
            .map(|(value, weight)| value * weight / 1000)
            .sum::<i32>();
        let mut tuned_full_white = weighted + tempo;
        if phase.endphase > ENDGAME_DRAW_SCALING_PHASE {
            let strong = if tuned_full_white > 0 {
                Color::White
            } else {
                Color::Black
            };
            let mul = self.get_draw_multiplier(strong);
            tuned_full_white = tuned_full_white * mul / DRAW_SCALING_DENOMINATOR;
        }

        HceFeatureBreakdown {
            values,
            tempo,
            phase_score,
            full_white,
            tuned_full_white,
        }
    }

    #[must_use]
    pub fn hce_feature_breakdown(&self) -> HceFeatureBreakdown {
        self.hce_feature_breakdown_with_pawn_structure(self.eval_pawn_structure())
    }

    fn evaluate_with_pawn_structure(&self, pawn_structure: (i32, i32)) -> i32 {
        let phase = PhaseFactors::from_game_phase(self.game_phase[0], self.game_phase[1]);

        // Base incremental scores (material + PST)
        let base_mg = self.eval_mg[0] - self.eval_mg[1];
        let base_eg = self.eval_eg[0] - self.eval_eg[1];

        // Accumulate all evaluation terms using EvalScore
        let mut total = EvalScore::new(base_mg, base_eg);
        total += EvalScore::both(self.bishop_bonus());
        total += self.advanced_eval_terms_with_pawn_structure(pawn_structure);

        let tempo = if self.white_to_move {
            TEMPO_BONUS
        } else {
            -TEMPO_BONUS
        };
        let mut score = phase.taper(total.mg, total.eg) + tempo;

        // Apply draw multiplier in endgames
        if phase.endphase > ENDGAME_DRAW_SCALING_PHASE {
            let strong = if score > 0 {
                Color::White
            } else {
                Color::Black
            };
            let mul = self.get_draw_multiplier(strong);
            score = score * mul / DRAW_SCALING_DENOMINATOR;
        }

        // Return from side-to-move perspective
        if self.white_to_move {
            score
        } else {
            -score
        }
    }

    /// Evaluate the position from the side-to-move's perspective.
    ///
    /// Uses tapered evaluation to interpolate between middlegame and endgame scores
    /// based on the current game phase. Includes all evaluation terms.
    #[must_use]
    pub fn evaluate(&self) -> i32 {
        self.evaluate_with_pawn_structure(self.eval_pawn_structure())
    }

    /// Evaluate full HCE while caching the pawn-only term.
    #[must_use]
    pub(crate) fn evaluate_cached(&self, pawn_hash: &crate::pawn_hash::PawnHashTable) -> i32 {
        self.evaluate_with_pawn_structure(self.eval_pawn_structure_cached(pawn_hash))
    }

    #[must_use]
    pub fn evaluate_tuned_hce(&self) -> i32 {
        self.evaluate_tuned_hce_with_pawn_structure(self.eval_pawn_structure())
    }

    fn evaluate_tuned_hce_with_pawn_structure(&self, pawn_structure: (i32, i32)) -> i32 {
        let score = self
            .hce_feature_breakdown_with_pawn_structure(pawn_structure)
            .tuned_full_white;
        if self.white_to_move {
            score
        } else {
            -score
        }
    }

    /// Evaluate tuned HCE while caching the pawn-only term.
    #[must_use]
    pub(crate) fn evaluate_tuned_hce_cached(
        &self,
        pawn_hash: &crate::pawn_hash::PawnHashTable,
    ) -> i32 {
        self.evaluate_tuned_hce_with_pawn_structure(self.eval_pawn_structure_cached(pawn_hash))
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

    /// Compute tactical NNUE features for expanded tactical networks.
    ///
    /// These features are intentionally non-incremental: the base piece-square
    /// accumulator stays cheap, and tactical rows are added from a cloned
    /// accumulator only when the loaded network supports them.
    fn push_nnue_tactical_square_feature(
        white_features: &mut Vec<usize>,
        black_features: &mut Vec<usize>,
        offset: usize,
        piece: Piece,
        color: Color,
        square: usize,
    ) {
        use super::nnue::network::tactical_piece_square_feature_index;

        white_features.push(tactical_piece_square_feature_index(
            offset,
            piece.index(),
            color.index(),
            square,
            0,
        ));
        black_features.push(tactical_piece_square_feature_index(
            offset,
            piece.index(),
            color.index(),
            square,
            1,
        ));
    }

    fn add_nnue_hanging_features(
        &self,
        ctx: &AttackContext,
        white_features: &mut Vec<usize>,
        black_features: &mut Vec<usize>,
    ) {
        use super::nnue::network::{NNUE_HANGING_OFFSET, NNUE_PAWN_ATTACKED_OFFSET};

        for color in Color::BOTH {
            let our_attacks = ctx.all_attacks(color);
            let their_attacks = ctx.all_attacks(color.opponent());
            let their_pawn_attacks = ctx.pawn_attacks(color.opponent());

            for piece in Piece::NON_KING {
                for sq in self.pieces_of(color, piece).iter() {
                    let sq_bb = 1u64 << sq.index();
                    let attacked = (sq_bb & their_attacks.0) != 0;
                    let defended = (sq_bb & our_attacks.0) != 0;
                    if attacked && !defended {
                        Self::push_nnue_tactical_square_feature(
                            white_features,
                            black_features,
                            NNUE_HANGING_OFFSET,
                            piece,
                            color,
                            sq.index(),
                        );
                    }
                    if (sq_bb & their_pawn_attacks.0) != 0 {
                        Self::push_nnue_tactical_square_feature(
                            white_features,
                            black_features,
                            NNUE_PAWN_ATTACKED_OFFSET,
                            piece,
                            color,
                            sq.index(),
                        );
                    }
                }
            }
        }
    }

    fn add_nnue_pinned_features(
        &self,
        white_features: &mut Vec<usize>,
        black_features: &mut Vec<usize>,
    ) {
        use super::nnue::network::NNUE_PINNED_OFFSET;

        for pinned_color in Color::BOTH {
            let king_sq = self.king_square_index(pinned_color);
            let opponent = pinned_color.opponent();

            for slider in [Piece::Bishop, Piece::Rook, Piece::Queen] {
                for slider_sq in self.pieces_of(opponent, slider).iter() {
                    let aligned = match slider {
                        Piece::Bishop => Self::aligned_on_ray(slider_sq.index(), king_sq, true),
                        Piece::Rook => Self::aligned_on_ray(slider_sq.index(), king_sq, false),
                        Piece::Queen => {
                            Self::aligned_on_ray(slider_sq.index(), king_sq, true)
                                || Self::aligned_on_ray(slider_sq.index(), king_sq, false)
                        }
                        Piece::Pawn | Piece::Knight | Piece::King => false,
                    };
                    if !aligned {
                        continue;
                    }
                    let between = Self::nnue_between_mask(slider_sq.index(), king_sq);
                    let blockers = self.all_occupied.0 & between;
                    if !blockers.is_power_of_two()
                        || blockers == 0
                        || (self.occupied_by(pinned_color).0 & blockers) == 0
                    {
                        continue;
                    }
                    let pinned_sq = blockers.trailing_zeros() as usize;
                    if let Some((_, piece)) = self.piece_at(Square::from_index(pinned_sq)) {
                        if piece != Piece::King {
                            Self::push_nnue_tactical_square_feature(
                                white_features,
                                black_features,
                                NNUE_PINNED_OFFSET,
                                piece,
                                pinned_color,
                                pinned_sq,
                            );
                        }
                    }
                }
            }
        }
    }

    fn add_nnue_check_features(
        &self,
        white_features: &mut Vec<usize>,
        black_features: &mut Vec<usize>,
    ) {
        use super::nnue::network::in_check_feature_index;

        for color in Color::BOTH {
            if self.is_in_check(color) {
                white_features.push(in_check_feature_index(color.index(), 0));
                black_features.push(in_check_feature_index(color.index(), 1));
            }
        }
    }

    fn nnue_tactical_piece_attacks(&self, square: usize, piece: Piece) -> u64 {
        match piece {
            Piece::Knight => KNIGHT_ATTACKS[square],
            Piece::Bishop => slider_attacks(square, self.all_occupied.0, true),
            Piece::Rook => slider_attacks(square, self.all_occupied.0, false),
            Piece::Queen => queen_attacks(square, self.all_occupied.0),
            Piece::Pawn | Piece::King => 0,
        }
    }

    fn add_nnue_king_pressure_features(
        &self,
        white_features: &mut Vec<usize>,
        black_features: &mut Vec<usize>,
    ) {
        use super::nnue::network::king_pressure_feature_index;

        for attacked_color in Color::BOTH {
            let attacker = attacked_color.opponent();
            let king_sq = self.king_square_index(attacked_color);
            let zone = KING_ZONE_EXTENDED[attacked_color.index()][king_sq].0;

            for piece in [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
                let pressure = self
                    .pieces_of(attacker, piece)
                    .iter()
                    .map(|sq| {
                        (self.nnue_tactical_piece_attacks(sq.index(), piece) & zone).count_ones()
                    })
                    .sum::<u32>();
                if pressure > 0 {
                    let bucket = pressure.min(7) as usize;
                    white_features.push(king_pressure_feature_index(
                        attacked_color.index(),
                        piece.index(),
                        bucket,
                        0,
                    ));
                    black_features.push(king_pressure_feature_index(
                        attacked_color.index(),
                        piece.index(),
                        bucket,
                        1,
                    ));
                }
            }
        }
    }

    fn add_nnue_mobility_features(
        &self,
        ctx: &AttackContext,
        white_features: &mut Vec<usize>,
        black_features: &mut Vec<usize>,
    ) {
        use super::nnue::network::mobility_feature_index;

        for color in Color::BOTH {
            let own = self.occupied_by(color).0;
            let enemy_pawn_attacks = ctx.pawn_attacks(color.opponent()).0;
            for piece in [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
                let mobility = self
                    .pieces_of(color, piece)
                    .iter()
                    .map(|sq| {
                        (self.nnue_tactical_piece_attacks(sq.index(), piece)
                            & !own
                            & !enemy_pawn_attacks)
                            .count_ones()
                    })
                    .sum::<u32>();
                if mobility > 0 {
                    let bucket = mobility.min(7) as usize;
                    white_features.push(mobility_feature_index(
                        color.index(),
                        piece.index(),
                        bucket,
                        0,
                    ));
                    black_features.push(mobility_feature_index(
                        color.index(),
                        piece.index(),
                        bucket,
                        1,
                    ));
                }
            }
        }
    }

    #[must_use]
    pub fn compute_nnue_dynamic_features(&self) -> (Vec<usize>, Vec<usize>) {
        let ctx = self.compute_attack_context();
        let mut white_features = Vec::with_capacity(32);
        let mut black_features = Vec::with_capacity(32);

        self.add_nnue_hanging_features(&ctx, &mut white_features, &mut black_features);
        self.add_nnue_pinned_features(&mut white_features, &mut black_features);
        self.add_nnue_check_features(&mut white_features, &mut black_features);
        self.add_nnue_king_pressure_features(&mut white_features, &mut black_features);
        self.add_nnue_mobility_features(&ctx, &mut white_features, &mut black_features);

        (white_features, black_features)
    }

    fn nnue_between_mask(sq1: usize, sq2: usize) -> u64 {
        let Some((file_step, rank_step)) = Self::nnue_line_step(sq1, sq2) else {
            return 0;
        };
        let file2 = sq2 % 8;
        let rank2 = sq2 / 8;
        let mut file = sq1 as i32 % 8 + file_step;
        let mut rank = sq1 as i32 / 8 + rank_step;
        let mut mask = 0u64;
        while file != file2 as i32 || rank != rank2 as i32 {
            if !(0..=7).contains(&file) || !(0..=7).contains(&rank) {
                return 0;
            }
            mask |= 1u64 << (rank * 8 + file);
            file += file_step;
            rank += rank_step;
        }
        mask
    }

    fn aligned_on_ray(sq1: usize, sq2: usize, diagonal: bool) -> bool {
        let file_delta = (sq2 % 8) as i32 - (sq1 % 8) as i32;
        let rank_delta = (sq2 / 8) as i32 - (sq1 / 8) as i32;
        if diagonal {
            file_delta.abs() == rank_delta.abs() && file_delta != 0
        } else {
            (file_delta == 0 || rank_delta == 0) && file_delta != rank_delta
        }
    }

    fn nnue_line_step(sq1: usize, sq2: usize) -> Option<(i32, i32)> {
        let file_delta = (sq2 % 8) as i32 - (sq1 % 8) as i32;
        let rank_delta = (sq2 / 8) as i32 - (sq1 / 8) as i32;
        if file_delta == 0 && rank_delta == 0 {
            None
        } else if file_delta == 0 {
            Some((0, rank_delta.signum()))
        } else if rank_delta == 0 {
            Some((file_delta.signum(), 0))
        } else if file_delta.abs() == rank_delta.abs() {
            Some((file_delta.signum(), rank_delta.signum()))
        } else {
            None
        }
    }

    pub(crate) fn add_nnue_dynamic_features(
        &self,
        acc: &mut super::nnue::NnueAccumulator,
        network: &super::nnue::NnueNetwork,
    ) {
        if !network.supports_tactical_features() {
            return;
        }

        let (white_features, black_features) = self.compute_nnue_dynamic_features();
        for (&white_feat, &black_feat) in white_features.iter().zip(&black_features) {
            if white_feat < network.input_size && black_feat < network.input_size {
                acc.add_feature(white_feat, black_feat, network);
            }
        }
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
        self.add_nnue_dynamic_features(&mut acc, network);

        // Evaluate
        network.evaluate(&acc, self.white_to_move)
    }
}
