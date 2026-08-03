use std::ops::Index;

use super::{Move, EMPTY_MOVE, MAX_MOVES};

/// List of moves with fixed-size backing array.
#[derive(Clone, Debug)]
pub struct MoveList {
    moves: [Move; MAX_MOVES],
    len: usize,
}

impl MoveList {
    pub(crate) fn new() -> Self {
        MoveList {
            moves: [EMPTY_MOVE; MAX_MOVES],
            len: 0,
        }
    }

    /// Append a move, silently discarding it if the list is already at
    /// capacity.
    ///
    /// `MAX_MOVES` comfortably exceeds the ~218-move ceiling for any legal
    /// chess position, so this never triggers during ordinary play. It can
    /// be reached from externally supplied FEN/EPD input (`position fen`,
    /// `setboard`) that packs far more sliding pieces onto the board than
    /// any legal game allows, which parses successfully -- piece counts per
    /// type are unbounded -- and can push pseudo-legal generation past 256
    /// moves before legality filtering thins the list. Dropping the
    /// overflow keeps that pathological-but-parseable input from crashing
    /// the engine process.
    pub(crate) fn push(&mut self, mv: Move) {
        if self.len < MAX_MOVES {
            self.moves[self.len] = mv;
            self.len += 1;
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub(crate) fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Move> {
        self.as_slice().iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Move> {
        self.as_mut_slice().iter_mut()
    }

    #[must_use]
    pub fn get(&self, idx: usize) -> Option<Move> {
        if idx < self.len {
            Some(self.moves[idx])
        } else {
            None
        }
    }

    #[must_use]
    pub fn first(&self) -> Option<Move> {
        self.get(0)
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = std::slice::Iter<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a> IntoIterator for &'a mut MoveList {
    type Item = &'a mut Move;
    type IntoIter = std::slice::IterMut<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

impl Default for MoveList {
    fn default() -> Self {
        MoveList::new()
    }
}

/// Owning iterator over moves in a `MoveList`.
pub struct MoveListIntoIter {
    list: MoveList,
    idx: usize,
}

impl Iterator for MoveListIntoIter {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.list.len {
            let mv = self.list.moves[self.idx];
            self.idx += 1;
            Some(mv)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.list.len - self.idx;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for MoveListIntoIter {}

impl IntoIterator for MoveList {
    type Item = Move;
    type IntoIter = MoveListIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        MoveListIntoIter { list: self, idx: 0 }
    }
}

impl Index<usize> for MoveList {
    type Output = Move;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(
            idx < self.len,
            "MoveList index {} out of bounds (len {})",
            idx,
            self.len
        );
        &self.moves[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::{MoveList, MAX_MOVES};
    use crate::board::{Move, Square};

    #[test]
    fn push_drops_moves_past_capacity_instead_of_panicking() {
        let mut list = MoveList::new();
        let mv = Move::quiet(Square::new(0, 0), Square::new(0, 1));
        for _ in 0..(MAX_MOVES + 10) {
            list.push(mv);
        }
        assert_eq!(list.len(), MAX_MOVES);
    }
}
