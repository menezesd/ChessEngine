use super::{Move, EMPTY_MOVE, MAX_MOVES};

/// A scored move for move ordering.
#[derive(Clone, Copy, Debug)]
pub struct ScoredMove {
    pub mv: Move,
    pub score: i32,
}

/// Fixed-size list of scored moves to avoid heap allocation.
#[derive(Clone, Debug)]
pub struct ScoredMoveList {
    moves: [ScoredMove; MAX_MOVES],
    len: usize,
}

impl ScoredMoveList {
    /// Create a new empty scored move list.
    #[must_use]
    pub fn new() -> Self {
        ScoredMoveList {
            moves: [ScoredMove {
                mv: EMPTY_MOVE,
                score: 0,
            }; MAX_MOVES],
            len: 0,
        }
    }

    /// Add a scored move to the list, silently discarding it if the list
    /// is already at capacity. See `MoveList::push` for why this can be
    /// reached (pathological FEN/EPD input) despite never happening in
    /// ordinary play.
    #[inline]
    pub fn push(&mut self, mv: Move, score: i32) {
        if self.len < MAX_MOVES {
            self.moves[self.len] = ScoredMove { mv, score };
            self.len += 1;
        }
    }

    /// Get the number of moves in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the list is empty.
    ///
    /// Required for API completeness with `len()` (`clippy::len_without_is_empty`).
    #[must_use]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get a slice of the scored moves.
    #[must_use]
    pub fn as_slice(&self) -> &[ScoredMove] {
        &self.moves[..self.len]
    }

    /// Get a mutable slice of the scored moves.
    pub fn as_mut_slice(&mut self) -> &mut [ScoredMove] {
        &mut self.moves[..self.len]
    }

    /// Sort moves by score in descending order.
    pub fn sort_by_score_desc(&mut self) {
        self.as_mut_slice()
            .sort_by_key(|scored| std::cmp::Reverse(scored.score));
    }

    /// Partial sort: find the best move from index `start` onwards and swap it to position `start`.
    /// Returns the move at position `start` after swapping (the best remaining move).
    /// This implements incremental selection sort - O(n-start) per call, but avoids sorting
    /// moves we'll never try due to early cutoffs.
    #[inline]
    pub fn pick_best(&mut self, start: usize) -> Option<&ScoredMove> {
        if start >= self.len {
            return None;
        }

        let mut best_idx = start;
        let mut best_score = self.moves[start].score;
        for i in (start + 1)..self.len {
            if self.moves[i].score > best_score {
                best_score = self.moves[i].score;
                best_idx = i;
            }
        }

        if best_idx != start {
            self.moves.swap(start, best_idx);
        }

        Some(&self.moves[start])
    }

    /// Iterate over scored moves.
    pub fn iter(&self) -> std::slice::Iter<'_, ScoredMove> {
        self.as_slice().iter()
    }
}

impl Default for ScoredMoveList {
    fn default() -> Self {
        ScoredMoveList::new()
    }
}
