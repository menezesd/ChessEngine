use smallvec::SmallVec;

use super::{Move, MAX_MOVES};

/// A scored move for move ordering.
#[derive(Clone, Copy, Debug)]
pub struct ScoredMove {
    pub mv: Move,
    pub score: i32,
}

/// Bounded inline storage that initializes only the appended entries.
#[derive(Clone, Debug)]
pub struct ScoredMoveList {
    moves: SmallVec<[ScoredMove; MAX_MOVES]>,
}

impl ScoredMoveList {
    /// Create a new empty scored move list.
    #[must_use]
    pub fn new() -> Self {
        ScoredMoveList {
            moves: SmallVec::new(),
        }
    }

    /// Add a scored move to the list, silently discarding it if the list
    /// is already at capacity. See `MoveList::push` for why this can be
    /// reached (pathological FEN/EPD input) despite never happening in
    /// ordinary play.
    #[inline]
    pub fn push(&mut self, mv: Move, score: i32) {
        if self.moves.len() < MAX_MOVES {
            self.moves.push(ScoredMove { mv, score });
        }
    }

    /// Get the number of moves in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.moves.len()
    }

    /// Check if the list is empty.
    ///
    /// Required for API completeness with `len()` (`clippy::len_without_is_empty`).
    #[must_use]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// Get a slice of the scored moves.
    #[must_use]
    pub fn as_slice(&self) -> &[ScoredMove] {
        self.moves.as_slice()
    }

    /// Get a mutable slice of the scored moves.
    pub fn as_mut_slice(&mut self) -> &mut [ScoredMove] {
        self.moves.as_mut_slice()
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
        let moves = self.moves.as_mut_slice();
        if start >= moves.len() {
            return None;
        }

        let mut best_idx = start;
        let mut best_score = moves[start].score;
        for (i, item) in moves.iter().enumerate().skip(start + 1) {
            if item.score > best_score {
                best_score = item.score;
                best_idx = i;
            }
        }

        if best_idx != start {
            moves.swap(start, best_idx);
        }

        Some(&moves[start])
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
