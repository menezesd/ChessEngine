use super::super::super::constants::SCORE_NEAR_MATE;

/// Correction history - tracks how wrong static eval is for similar pawn structures.
/// When search finds a more accurate score than static eval, we store the correction
/// indexed by pawn hash. Future positions with similar pawn structures get this
/// correction applied to their static eval.
const CORRECTION_HISTORY_SIZE: usize = 16384;
const MIN_CORRECTION_DEPTH: u32 = 2;
const MAX_CORRECTION_DEPTH: u32 = 8;
const CORRECTION_WEIGHT_SCALE: i32 = 256;
const CORRECTION_WEIGHT_PER_DEPTH: u32 = 8;
const CORRECTION_DELTA_LIMIT: i32 = 500;
const CORRECTION_VALUE_LIMIT: i32 = 1000;

pub struct CorrectionHistory {
    /// Indexed by `pawn_hash` % size, stores weighted average correction
    corrections: Box<[i16; CORRECTION_HISTORY_SIZE]>,
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
            corrections: Box::new([0; CORRECTION_HISTORY_SIZE]),
        }
    }

    fn index(pawn_hash: u64) -> usize {
        (pawn_hash as usize) % CORRECTION_HISTORY_SIZE
    }

    /// Get correction for a position based on pawn structure
    #[must_use]
    pub fn get(&self, pawn_hash: u64) -> i32 {
        self.corrections[Self::index(pawn_hash)] as i32
    }

    /// Update correction when we find search score differs from static eval
    /// Uses exponential moving average: new = old * (1-weight) + correction * weight
    pub fn update(&mut self, pawn_hash: u64, static_eval: i32, search_score: i32, depth: u32) {
        if depth < MIN_CORRECTION_DEPTH || search_score.abs() > SCORE_NEAR_MATE {
            return;
        }

        let correction = search_score
            .saturating_sub(static_eval)
            .clamp(-CORRECTION_DELTA_LIMIT, CORRECTION_DELTA_LIMIT);
        let idx = Self::index(pawn_hash);
        let old = self.corrections[idx] as i32;
        let weight = correction_weight(depth);
        let new_val = (old * (CORRECTION_WEIGHT_SCALE - weight) + correction * weight)
            / CORRECTION_WEIGHT_SCALE;

        self.corrections[idx] =
            new_val.clamp(-CORRECTION_VALUE_LIMIT, CORRECTION_VALUE_LIMIT) as i16;
    }

    /// Reset all corrections
    pub fn reset(&mut self) {
        self.corrections.fill(0);
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

        history.update(123, 0, 400, 1);

        assert_eq!(history.get(123), 0);
    }

    #[test]
    fn correction_history_applies_weighted_delta() {
        let mut history = CorrectionHistory::new();

        history.update(123, 100, 356, 4);

        assert_eq!(history.get(123), 32);
    }

    #[test]
    fn correction_weight_caps_at_max_depth() {
        assert_eq!(correction_weight(8), correction_weight(32));
    }

    #[test]
    fn correction_history_saturates_extreme_eval_delta() {
        let mut history = CorrectionHistory::new();

        history.update(123, i32::MIN, 0, 4);

        assert_eq!(history.get(123), 62);
    }
}
