use crate::board::Move;

const HISTORY_SIZE: usize = 4096;

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
            *entry = entry.saturating_add(history_bonus(depth));
        }
    }

    /// Penalize a move that failed to cause a cutoff (negative history)
    pub fn penalize(&mut self, mv: &Move, depth: u32) {
        if let Some(entry) = self.entries.get_mut(mv.history_index()) {
            *entry = entry.saturating_sub(history_penalty(depth));
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
    use super::{history_bonus, history_penalty};

    #[test]
    fn history_bonus_saturates_extreme_depth() {
        assert_eq!(history_bonus(u32::MAX), i32::MAX);
    }

    #[test]
    fn history_penalty_saturates_extreme_depth() {
        assert_eq!(history_penalty(u32::MAX), i32::MAX);
    }
}
