use super::super::super::constants::SCORE_NEAR_MATE;
use crate::board::Color;

/// Correction history - tracks how wrong static eval is for similar pawn structures.
/// When search finds a more accurate score than static eval, we store the correction
/// indexed by pawn hash and side to move. Scores are side-relative, so each
/// side learns its own correction for a given pawn structure.
const CORRECTION_HISTORY_SIZE: usize = 16384;
const MIN_CORRECTION_DEPTH: u32 = 2;
const MAX_CORRECTION_DEPTH: u32 = 8;
const CORRECTION_WEIGHT_SCALE: i32 = 256;
const CORRECTION_WEIGHT_PER_DEPTH: u32 = 8;
const CORRECTION_DELTA_LIMIT: i32 = 500;
const CORRECTION_VALUE_LIMIT: i32 = 1000;

pub struct CorrectionHistory {
    /// Indexed by side to move, then `pawn_hash` % size.
    corrections: [Box<[i16; CORRECTION_HISTORY_SIZE]>; 2],
}

impl Default for CorrectionHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl CorrectionHistory {
    #[must_use]
    pub fn new() -> Self {
        CorrectionHistory {
            corrections: std::array::from_fn(|_| Box::new([0; CORRECTION_HISTORY_SIZE])),
        }
    }

    fn index(pawn_hash: u64) -> usize {
        (pawn_hash as usize) % CORRECTION_HISTORY_SIZE
    }

    /// Get correction for a position based on pawn structure
    #[must_use]
    pub fn get(&self, pawn_hash: u64, side: Color) -> i32 {
        self.corrections[side.index()][Self::index(pawn_hash)] as i32
    }

    /// Update correction when we find search score differs from static eval
    /// Uses exponential moving average: new = old * (1-weight) + correction * weight
    pub fn update(
        &mut self,
        pawn_hash: u64,
        side: Color,
        static_eval: i32,
        search_score: i32,
        depth: u32,
    ) {
        if depth < MIN_CORRECTION_DEPTH || search_score.abs() > SCORE_NEAR_MATE {
            return;
        }

        let correction = search_score
            .saturating_sub(static_eval)
            .clamp(-CORRECTION_DELTA_LIMIT, CORRECTION_DELTA_LIMIT);
        let idx = Self::index(pawn_hash);
        let entry = &mut self.corrections[side.index()][idx];
        let old = i32::from(*entry);
        let weight = correction_weight(depth);
        let new_val = (old * (CORRECTION_WEIGHT_SCALE - weight) + correction * weight)
            / CORRECTION_WEIGHT_SCALE;

        *entry = new_val.clamp(-CORRECTION_VALUE_LIMIT, CORRECTION_VALUE_LIMIT) as i16;
    }

    /// Reset all corrections
    pub fn reset(&mut self) {
        for side in &mut self.corrections {
            side.fill(0);
        }
    }
}

fn correction_weight(depth: u32) -> i32 {
    (depth.min(MAX_CORRECTION_DEPTH) * CORRECTION_WEIGHT_PER_DEPTH) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_history_ignores_shallow_updates() {
        let mut history = CorrectionHistory::new();

        history.update(123, Color::White, 0, 400, 1);

        assert_eq!(history.get(123, Color::White), 0);
    }

    #[test]
    fn correction_history_applies_weighted_delta() {
        let mut history = CorrectionHistory::new();

        history.update(123, Color::White, 100, 356, 4);

        assert_eq!(history.get(123, Color::White), 32);
    }

    #[test]
    fn correction_weight_caps_at_max_depth() {
        assert_eq!(correction_weight(8), correction_weight(32));
    }

    #[test]
    fn correction_history_saturates_extreme_eval_delta() {
        let mut history = CorrectionHistory::new();

        history.update(123, Color::White, i32::MIN, 0, 4);

        assert_eq!(history.get(123, Color::White), 62);
    }

    #[test]
    fn corrections_for_opposite_sides_do_not_cancel_each_other() {
        let mut history = CorrectionHistory::new();
        history.update(123, Color::White, 0, 400, 8);
        history.update(123, Color::Black, 0, -400, 8);

        assert_eq!(history.get(123, Color::White), 100);
        assert_eq!(history.get(123, Color::Black), -100);

        history.reset();
        assert_eq!(history.get(123, Color::White), 0);
        assert_eq!(history.get(123, Color::Black), 0);
    }
}
