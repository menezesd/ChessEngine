//! Position evaluation using tapered eval.
//!
//! Uses incremental evaluation stored on the board.
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

use score::PhaseFactors;

/// Bishop pair bonus in centipawns (Texel tuned v2)
const BISHOP_PAIR_BONUS: i32 = 18;

/// Tempo bonus (side to move advantage) (Texel tuned v2)
const TEMPO_BONUS: i32 = 19;

/// Bishop vs Knight imbalance bonus per pawn difference from 8.
/// Bishops are worth more in open positions (fewer pawns).
/// Scales the bishop advantage by the number of pawns off the board.
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

const NNUE_ACTIVITY_PIECES: [Piece; 4] = [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen];

struct NnueActivity {
    attacks: AttackContext,
    pressure: [[u32; 4]; 2],
    mobility: [[u32; 4]; 2],
}

fn bishop_pair_bonus(ours: u32, theirs: u32) -> i32 {
    BISHOP_PAIR_BONUS * ((ours / 2) as i32 - (theirs / 2) as i32)
}

impl Board {
    fn bishop_bonus(&self) -> i32 {
        // Bishop pair bonus
        let white_bishops = self.pieces_of(Color::White, Piece::Bishop).popcount();
        let black_bishops = self.pieces_of(Color::Black, Piece::Bishop).popcount();
        let bishop_pair_bonus = bishop_pair_bonus(white_bishops, black_bishops);

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

    fn hce_terms(&self, pawn_structure: (i32, i32)) -> [(i32, i32); HCE_FEATURE_NAMES.len()] {
        let ctx = self.compute_attack_context();
        let bishop_bonus = self.bishop_bonus();
        let (passed_mg, passed_eg, hanging) = self.eval_attacks_dependent_with_context(&ctx);
        // Values follow HCE_FEATURE_NAMES and TUNED_HCE_WEIGHTS_PERMILLE.
        [
            (
                self.eval.mg[0] - self.eval.mg[1],
                self.eval.eg[0] - self.eval.eg[1],
            ),
            (bishop_bonus, bishop_bonus),
            self.eval_mobility_with_context(&ctx),
            pawn_structure,
            self.eval_king_safety_with_context(&ctx),
            self.eval_king_shield(),
            self.eval_rooks(),
            self.eval_minor_pieces(&ctx),
            (self.eval_tropism(), 0),
            (passed_mg, passed_eg),
            (hanging, hanging),
            self.eval_coordination(&ctx),
            self.eval_pawn_advanced(),
            self.eval_weak_squares(&ctx),
            self.eval_king_danger(&ctx),
            self.eval_endgame_patterns(),
            self.eval_space_control(&ctx),
            self.eval_threats_advanced(),
            self.eval_piece_quality(&ctx),
            self.eval_imbalances(),
            self.eval_initiative(&ctx),
        ]
    }

    fn scale_hce_for_draws(&self, phase: PhaseFactors, score: i32) -> i32 {
        if phase.endphase <= ENDGAME_DRAW_SCALING_PHASE {
            return score;
        }
        let strong = if score > 0 {
            Color::White
        } else {
            Color::Black
        };
        score * self.get_draw_multiplier(strong) / DRAW_SCALING_DENOMINATOR
    }

    fn weight_hce_terms(values: [i32; HCE_FEATURE_NAMES.len()]) -> i32 {
        values
            .into_iter()
            .zip(TUNED_HCE_WEIGHTS_PERMILLE)
            .map(|(value, weight)| value * weight / 1000)
            .sum()
    }

    fn hce_feature_breakdown_with_pawn_structure(
        &self,
        pawn_structure: (i32, i32),
    ) -> HceFeatureBreakdown {
        let phase = PhaseFactors::from_game_phase(self.eval.phase);
        let terms = self.hce_terms(pawn_structure);
        let phase_score = phase.taper_total(terms);
        let values = terms.map(|term| phase.taper(term));
        let tempo = self.side_to_move().sign() * TEMPO_BONUS;

        HceFeatureBreakdown {
            values,
            tempo,
            phase_score,
            full_white: self.scale_hce_for_draws(phase, phase_score + tempo),
            tuned_full_white: self
                .scale_hce_for_draws(phase, Self::weight_hce_terms(values) + tempo),
        }
    }

    #[must_use]
    pub fn hce_feature_breakdown(&self) -> HceFeatureBreakdown {
        self.hce_feature_breakdown_with_pawn_structure(self.eval_pawn_structure())
    }

    fn evaluate_with_pawn_structure(&self, pawn_structure: (i32, i32)) -> i32 {
        let phase = PhaseFactors::from_game_phase(self.eval.phase);
        let sign = self.side_to_move().sign();
        let raw = phase.taper_total(self.hce_terms(pawn_structure)) + sign * TEMPO_BONUS;
        sign * self.scale_hce_for_draws(phase, raw)
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
        let phase = PhaseFactors::from_game_phase(self.eval.phase);
        let sign = self.side_to_move().sign();
        let values = self.hce_terms(pawn_structure).map(|term| phase.taper(term));
        let raw = Self::weight_hce_terms(values) + sign * TEMPO_BONUS;
        sign * self.scale_hce_for_draws(phase, raw)
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
    /// Uses specialized scores for rook versus pawn and rook and pawn versus
    /// rook. Other positions use incremental material and piece-square
    /// scores, plus the bishop pair.
    #[must_use]
    pub fn evaluate_simple(&self) -> i32 {
        let stm = self.side_to_move();
        if let Some(score) = self.single_pawn_rook_eval() {
            return stm.sign() * score + TEMPO_BONUS;
        }
        let stm_idx = stm.index();
        let opp_idx = stm.opponent().index();

        let phase = PhaseFactors::from_game_phase(self.eval.phase);

        let mideval = self.eval.mg[stm_idx] - self.eval.mg[opp_idx];
        let endeval = self.eval.eg[stm_idx] - self.eval.eg[opp_idx];

        // Bishop pair bonus only (imbalance is in full eval)
        let our_bishops = self.pieces_of(stm, Piece::Bishop).popcount();
        let opp_bishops = self.opponent_pieces(stm, Piece::Bishop).popcount();
        let bishop_bonus = bishop_pair_bonus(our_bishops, opp_bishops);

        phase.taper((mideval, endeval)) + bishop_bonus + TEMPO_BONUS
    }

    /// Compute active NNUE features for both perspectives.
    /// Returns (`white_features`, `black_features`) as vectors of feature indices.
    #[must_use]
    pub fn compute_nnue_features(&self) -> (Vec<usize>, Vec<usize>) {
        let mut white_features = Vec::with_capacity(32);
        let mut black_features = Vec::with_capacity(32);
        self.visit_nnue_features(|white, black| {
            white_features.push(white);
            black_features.push(black);
        });
        (white_features, black_features)
    }

    fn visit_nnue_features(&self, mut emit: impl FnMut(usize, usize)) {
        use super::nnue::network::feature_index;

        for color in Color::BOTH {
            let color_idx = color.index();
            for piece in Piece::ALL {
                let piece_idx = piece.index();
                for sq in self.pieces_of(color, piece).iter() {
                    let sq_idx = sq.as_index();
                    emit(
                        feature_index(piece_idx, color_idx, sq_idx, 0),
                        feature_index(piece_idx, color_idx, sq_idx, 1),
                    );
                }
            }
        }
    }

    pub(super) fn nnue_accumulator(
        &self,
        network: &super::nnue::NnueNetwork,
    ) -> super::nnue::NnueAccumulator {
        let mut accumulator = super::nnue::NnueAccumulator::new(&network.feature_bias);
        self.visit_nnue_features(|white, black| accumulator.add_feature(white, black, network));
        accumulator
    }

    /// Compute tactical NNUE features for expanded tactical networks.
    ///
    /// These features are intentionally non-incremental: the base piece-square
    /// accumulator stays cheap, and tactical rows are added from a cloned
    /// accumulator only when the loaded network supports them.
    fn visit_nnue_tactical_square_feature(
        emit: &mut impl FnMut(usize, usize),
        offset: usize,
        piece: Piece,
        color: Color,
        square: usize,
    ) {
        use super::nnue::network::tactical_piece_square_feature_index;

        emit(
            tactical_piece_square_feature_index(offset, piece.index(), color.index(), square, 0),
            tactical_piece_square_feature_index(offset, piece.index(), color.index(), square, 1),
        );
    }

    fn visit_nnue_hanging_features(
        &self,
        ctx: &AttackContext,
        emit: &mut impl FnMut(usize, usize),
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
                        Self::visit_nnue_tactical_square_feature(
                            emit,
                            NNUE_HANGING_OFFSET,
                            piece,
                            color,
                            sq.index(),
                        );
                    }
                    if (sq_bb & their_pawn_attacks.0) != 0 {
                        Self::visit_nnue_tactical_square_feature(
                            emit,
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

    fn visit_nnue_pinned_features(&self, emit: &mut impl FnMut(usize, usize)) {
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
                            Self::visit_nnue_tactical_square_feature(
                                emit,
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

    fn visit_nnue_check_features(&self, emit: &mut impl FnMut(usize, usize)) {
        use super::nnue::network::in_check_feature_index;

        for color in Color::BOTH {
            if self.is_in_check(color) {
                emit(
                    in_check_feature_index(color.index(), 0),
                    in_check_feature_index(color.index(), 1),
                );
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

    fn nnue_activity(&self) -> NnueActivity {
        let pawns = [
            self.pawn_attacks(Color::White),
            self.pawn_attacks(Color::Black),
        ];
        let mut all = pawns;
        let mut pressure = [[0; 4]; 2];
        let mut mobility = [[0; 4]; 2];
        for color in Color::BOTH {
            let side = color.index();
            let defender = color.opponent();
            let defended_side = defender.index();
            let safe_mobility = !(self.occupied_by(color).0 | pawns[defended_side].0);
            let zone = KING_ZONE_EXTENDED[defended_side][self.king_square_index(defender)].0;
            for king in self.pieces_of(color, Piece::King).iter() {
                all[side].0 |= super::attack_tables::KING_ATTACKS[king.index()];
            }
            for (index, piece) in NNUE_ACTIVITY_PIECES.into_iter().enumerate() {
                for square in self.pieces_of(color, piece).iter() {
                    let attacks = self.nnue_tactical_piece_attacks(square.index(), piece);
                    all[side].0 |= attacks;
                    pressure[defended_side][index] += (attacks & zone).count_ones();
                    mobility[side][index] += (attacks & safe_mobility).count_ones();
                }
            }
        }
        NnueActivity {
            attacks: AttackContext {
                white_attacks: all[Color::White.index()],
                black_attacks: all[Color::Black.index()],
                white_pawn_attacks: pawns[Color::White.index()],
                black_pawn_attacks: pawns[Color::Black.index()],
            },
            pressure,
            mobility,
        }
    }

    fn visit_nnue_activity_features(
        counts: &[[u32; 4]; 2],
        feature_index: fn(usize, usize, usize, usize) -> usize,
        emit: &mut impl FnMut(usize, usize),
    ) {
        for color in Color::BOTH {
            for (index, piece) in NNUE_ACTIVITY_PIECES.into_iter().enumerate() {
                let count = counts[color.index()][index];
                if count != 0 {
                    let bucket = count.min(7) as usize;
                    emit(
                        feature_index(color.index(), piece.index(), bucket, 0),
                        feature_index(color.index(), piece.index(), bucket, 1),
                    );
                }
            }
        }
    }

    fn visit_nnue_dynamic_features(&self, mut emit: impl FnMut(usize, usize)) {
        use super::nnue::network::{king_pressure_feature_index, mobility_feature_index};
        let activity = self.nnue_activity();
        self.visit_nnue_hanging_features(&activity.attacks, &mut emit);
        self.visit_nnue_pinned_features(&mut emit);
        self.visit_nnue_check_features(&mut emit);
        Self::visit_nnue_activity_features(
            &activity.pressure,
            king_pressure_feature_index,
            &mut emit,
        );
        Self::visit_nnue_activity_features(&activity.mobility, mobility_feature_index, &mut emit);
    }

    #[must_use]
    pub fn compute_nnue_dynamic_features(&self) -> (Vec<usize>, Vec<usize>) {
        let mut white_features = Vec::with_capacity(32);
        let mut black_features = Vec::with_capacity(32);
        self.visit_nnue_dynamic_features(|white, black| {
            white_features.push(white);
            black_features.push(black);
        });
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

        self.visit_nnue_dynamic_features(|white, black| {
            if white < network.input_size && black < network.input_size {
                acc.add_feature(white, black, network);
            }
        });
    }

    /// Evaluate position using NNUE network.
    /// Returns score in centipawns from side-to-move perspective.
    #[must_use]
    pub fn evaluate_nnue(&self, network: &super::nnue::NnueNetwork) -> i32 {
        let mut acc = self.nnue_accumulator(network);
        self.add_nnue_dynamic_features(&mut acc, network);

        network.evaluate(&acc, self.white_to_move)
    }
}
