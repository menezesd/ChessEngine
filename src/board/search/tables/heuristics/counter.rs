use crate::board::{Move, Piece, EMPTY_MOVE};

const BOARD_SQUARES: usize = 64;
const HISTORY_OUTER_SIZE: usize = 384;
const HISTORY_INNER_SIZE: usize = BOARD_SQUARES * BOARD_SQUARES;
const HISTORY_MAX: i16 = 16000;

pub struct CounterMoveTable {
    entries: [[Move; BOARD_SQUARES]; BOARD_SQUARES],
}

impl Default for CounterMoveTable {
    fn default() -> Self {
        Self::new()
    }
}

impl CounterMoveTable {
    #[must_use]
    pub fn new() -> Self {
        CounterMoveTable {
            entries: [[EMPTY_MOVE; BOARD_SQUARES]; BOARD_SQUARES],
        }
    }

    #[must_use]
    pub fn get(&self, from: usize, to: usize) -> Move {
        if from < BOARD_SQUARES && to < BOARD_SQUARES {
            self.entries[from][to]
        } else {
            EMPTY_MOVE
        }
    }

    pub fn set(&mut self, from: usize, to: usize, mv: Move) {
        if from < BOARD_SQUARES && to < BOARD_SQUARES {
            self.entries[from][to] = mv;
        }
    }

    pub fn reset(&mut self) {
        for counters in &mut self.entries {
            counters.fill(EMPTY_MOVE);
        }
    }
}

/// Continuation history table - tracks what moves work well after previous moves.
///
/// Indexed by `[prev_piece][prev_to][curr_from][curr_to]` simplified to
/// `[prev_piece * 64 + prev_to][curr_from * 64 + curr_to]`.
/// We use 6 piece types * 64 squares = 384 outer slots, each with 4096 inner entries.
pub struct ContinuationHistory {
    /// [piece * 64 + to] -> [from * 64 + to] -> score
    entries: Box<[[i16; HISTORY_INNER_SIZE]; HISTORY_OUTER_SIZE]>,
}

impl Default for ContinuationHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContinuationHistory {
    #[must_use]
    pub fn new() -> Self {
        ContinuationHistory {
            entries: Box::new([[0i16; HISTORY_INNER_SIZE]; HISTORY_OUTER_SIZE]),
        }
    }

    /// Get continuation history score for a move following a previous move
    #[must_use]
    pub fn score(&self, prev_piece: Piece, prev_to: usize, mv: Move) -> i32 {
        let outer_idx = piece_square_index(prev_piece, prev_to);
        let inner_idx = move_index(mv);
        if outer_idx < HISTORY_OUTER_SIZE {
            i32::from(self.entries[outer_idx][inner_idx])
        } else {
            0
        }
    }

    /// Update continuation history on beta cutoff
    pub fn update(&mut self, prev_piece: Piece, prev_to: usize, mv: Move, depth: u32) {
        let outer_idx = piece_square_index(prev_piece, prev_to);
        let inner_idx = move_index(mv);
        if outer_idx < HISTORY_OUTER_SIZE {
            apply_history_bonus(&mut self.entries[outer_idx][inner_idx], depth);
        }
    }

    /// Decay all entries
    pub fn decay(&mut self) {
        for outer in self.entries.iter_mut() {
            for entry in outer.iter_mut() {
                *entry >>= 2;
            }
        }
    }

    /// Reset all entries
    pub fn reset(&mut self) {
        for outer in self.entries.iter_mut() {
            for entry in outer.iter_mut() {
                *entry = 0;
            }
        }
    }
}

/// Countermove history table - tracks what responses work well against opponent moves.
///
/// Unlike continuation history (which uses our previous move), this uses the opponent's
/// previous move (`prev_piece`, `prev_to`) and our current move's piece type.
/// Indexed by `[opp_piece * 64 + opp_to][our_piece * 64 + our_to]`.
pub struct CountermoveHistory {
    /// `[prev_piece * 64 + prev_to]` -> `[piece * 64 + to]` -> score
    entries: Box<[[i16; HISTORY_INNER_SIZE]; HISTORY_OUTER_SIZE]>,
}

impl Default for CountermoveHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl CountermoveHistory {
    #[must_use]
    pub fn new() -> Self {
        CountermoveHistory {
            entries: Box::new([[0i16; HISTORY_INNER_SIZE]; HISTORY_OUTER_SIZE]),
        }
    }

    /// Get countermove history score for responding to opponent's move
    #[must_use]
    pub fn score(&self, opp_piece: Piece, opp_to: usize, our_piece: Piece, mv: Move) -> i32 {
        let outer_idx = piece_square_index(opp_piece, opp_to);
        let inner_idx = piece_square_index(our_piece, mv.to().index());
        if outer_idx < HISTORY_OUTER_SIZE {
            i32::from(self.entries[outer_idx][inner_idx])
        } else {
            0
        }
    }

    /// Update countermove history on beta cutoff
    pub fn update(
        &mut self,
        opp_piece: Piece,
        opp_to: usize,
        our_piece: Piece,
        mv: Move,
        depth: u32,
    ) {
        let outer_idx = piece_square_index(opp_piece, opp_to);
        let inner_idx = piece_square_index(our_piece, mv.to().index());
        if outer_idx < HISTORY_OUTER_SIZE {
            apply_history_bonus(&mut self.entries[outer_idx][inner_idx], depth);
        }
    }

    /// Decay all entries
    pub fn decay(&mut self) {
        for outer in self.entries.iter_mut() {
            for entry in outer.iter_mut() {
                *entry >>= 2;
            }
        }
    }

    /// Reset all entries
    pub fn reset(&mut self) {
        for outer in self.entries.iter_mut() {
            for entry in outer.iter_mut() {
                *entry = 0;
            }
        }
    }
}

fn piece_square_index(piece: Piece, square: usize) -> usize {
    piece as usize * BOARD_SQUARES + square
}

fn move_index(mv: Move) -> usize {
    mv.from().index() * BOARD_SQUARES + mv.to().index()
}

fn apply_history_bonus(entry: &mut i16, depth: u32) {
    let bonus = depth.saturating_mul(depth).min(HISTORY_MAX as u32) as i16;
    *entry = entry.saturating_add(bonus).min(HISTORY_MAX);
}

#[cfg(test)]
mod tests {
    use super::{apply_history_bonus, HISTORY_MAX};

    #[test]
    fn apply_history_bonus_saturates_extreme_depth() {
        let mut entry = 0;

        apply_history_bonus(&mut entry, u32::MAX);

        assert_eq!(entry, HISTORY_MAX);
    }
}
