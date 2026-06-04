use crate::board::Piece;

const PIECE_TYPE_COUNT: usize = 6;
const CAPTURE_HISTORY_MAX: i32 = 50_000;

/// Capture history table - tracks which captures historically cause cutoffs.
/// Indexed by `[attacker_piece][victim_piece]` for a 6x6 = 36 entry table.
pub struct CaptureHistory {
    entries: [[i32; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT],
}

impl Default for CaptureHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHistory {
    #[must_use]
    pub fn new() -> Self {
        CaptureHistory {
            entries: [[0; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT],
        }
    }

    /// Get capture history score for a capture move
    #[must_use]
    pub fn score(&self, attacker: Piece, victim: Piece) -> i32 {
        self.entries[attacker as usize][victim as usize]
    }

    /// Update capture history on beta cutoff
    pub fn update(&mut self, attacker: Piece, victim: Piece, depth: u32) {
        let entry = &mut self.entries[attacker as usize][victim as usize];
        *entry = entry
            .saturating_add(capture_history_bonus(depth))
            .min(CAPTURE_HISTORY_MAX);
    }

    /// Decay all entries
    pub fn decay(&mut self) {
        for row in &mut self.entries {
            for entry in row {
                *entry >>= 2;
            }
        }
    }

    /// Reset all entries
    pub fn reset(&mut self) {
        self.entries = [[0; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT];
    }
}

fn capture_history_bonus(depth: u32) -> i32 {
    depth
        .saturating_mul(depth)
        .saturating_mul(depth)
        .min(i32::MAX as u32) as i32
}

#[cfg(test)]
mod tests {
    use super::capture_history_bonus;

    #[test]
    fn capture_history_bonus_saturates_extreme_depth() {
        assert_eq!(capture_history_bonus(u32::MAX), i32::MAX);
    }
}
