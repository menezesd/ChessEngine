use crate::board::Move;

const HISTORY_SIZE: usize = 4096;

/// Bound for history scores. Keeps quiet-move ordering scores below the
/// capture and TT-move bands and matches the cap used by the continuation
/// and countermove history tables.
const MAX_HISTORY: i32 = 16000;

pub struct HistoryTable {
    entries: [i32; HISTORY_SIZE],
}

impl Default for HistoryTable {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryTable {
    #[must_use]
    pub fn new() -> Self {
        HistoryTable {
            entries: [0; HISTORY_SIZE],
        }
    }

    #[must_use]
    pub fn score(&self, mv: &Move) -> i32 {
        self.entries.get(mv.history_index()).copied().unwrap_or(0)
    }

    /// Update history score for a move that caused a beta cutoff
    pub fn update(&mut self, mv: &Move, depth: u32) {
        if let Some(entry) = self.entries.get_mut(mv.history_index()) {
            apply_gravity(entry, history_bonus(depth));
        }
    }

    /// Penalize a move that failed to cause a cutoff (negative history)
    pub fn penalize(&mut self, mv: &Move, depth: u32) {
        if let Some(entry) = self.entries.get_mut(mv.history_index()) {
            apply_gravity(entry, -history_penalty(depth));
        }
    }

    pub fn decay(&mut self) {
        for entry in &mut self.entries {
            *entry >>= 2;
        }
    }

    pub fn reset(&mut self) {
        self.entries = [0; HISTORY_SIZE];
    }
}

/// History gravity: pull the entry toward the (signed) bonus so scores stay
/// within `[-MAX_HISTORY, MAX_HISTORY]`. Without the proportional decay term,
/// hot moves in a long search grow without bound and outrank the killer,
/// capture, and TT-move ordering bands, and every quiet move eventually
/// clears `LMR_SCORE_THRESHOLD`, disabling late move reductions.
fn apply_gravity(entry: &mut i32, bonus: i32) {
    let bonus = bonus.clamp(-MAX_HISTORY, MAX_HISTORY);
    *entry += bonus - *entry * bonus.abs() / MAX_HISTORY;
}

fn history_bonus(depth: u32) -> i32 {
    depth
        .saturating_mul(depth)
        .saturating_mul(depth)
        .min(i32::MAX as u32) as i32
}

fn history_penalty(depth: u32) -> i32 {
    depth.saturating_mul(depth).min(i32::MAX as u32) as i32
}

#[cfg(test)]
mod tests {
    use super::{apply_gravity, history_bonus, history_penalty, MAX_HISTORY};

    #[test]
    fn history_bonus_saturates_extreme_depth() {
        assert_eq!(history_bonus(u32::MAX), i32::MAX);
    }

    #[test]
    fn history_penalty_saturates_extreme_depth() {
        assert_eq!(history_penalty(u32::MAX), i32::MAX);
    }

    #[test]
    fn gravity_bounds_repeated_bonuses() {
        let mut entry = 0;
        for _ in 0..1_000 {
            apply_gravity(&mut entry, history_bonus(20));
        }
        assert!(entry <= MAX_HISTORY, "entry {entry} exceeded MAX_HISTORY");

        for _ in 0..1_000 {
            apply_gravity(&mut entry, -history_penalty(20));
        }
        assert!(entry >= -MAX_HISTORY, "entry {entry} fell below -MAX_HISTORY");
    }

    #[test]
    fn gravity_moves_entry_toward_bonus() {
        let mut entry = 0;
        apply_gravity(&mut entry, 1_000);
        assert_eq!(entry, 1_000);

        apply_gravity(&mut entry, -1_000);
        assert!(entry < 1_000);
    }
}
