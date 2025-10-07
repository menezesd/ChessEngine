use std::mem;

use crate::types::Move;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Debug)]
pub struct TTEntry {
    pub hash: u64,
    pub depth: u32,
    pub score: i32,
    pub bound_type: BoundType,
    pub best_move: Option<Move>,
}

pub struct TranspositionTable {
    table: Vec<Option<TTEntry>>,
    mask: usize,
}

impl TranspositionTable {
    /// Create a new transposition table sized approximately `size_mb` megabytes.
    ///
    /// The function computes a power-of-two number of entries and returns a
    /// simple direct-mapped table used for storing TT entries.
    pub fn new(size_mb: usize) -> Self {
        let entry_size = mem::size_of::<Option<TTEntry>>();
        let mut num_entries = (size_mb * 1024 * 1024) / entry_size;

        num_entries = num_entries.next_power_of_two() / 2;
        if num_entries == 0 {
            num_entries = 1024;
        }

        TranspositionTable {
            table: vec![None; num_entries],
            mask: num_entries - 1,
        }
    }

    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    pub fn probe(&self, hash: u64) -> Option<&TTEntry> {
        let index = self.index(hash);
        if let Some(entry) = &self.table[index] {
            if entry.hash == hash {
                return Some(entry);
            }
        }
        None
    }

    /// Store or update a transposition-table entry for `hash`.
    ///
    /// Replaces an existing entry at the same index if the incoming `depth`
    /// is greater-or-equal to the existing entry's depth.
    pub fn store(
        &mut self,
        hash: u64,
        depth: u32,
        score: i32,
        bound_type: BoundType,
        best_move: Option<Move>,
    ) {
        let index = self.index(hash);
        let should_replace = match &self.table[index] {
            Some(existing_entry) => depth >= existing_entry.depth,
            None => true,
        };

        if should_replace {
            self.table[index] = Some(TTEntry {
                hash,
                depth,
                score,
                bound_type,
                best_move,
            });
        }
    }
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new(1024)
    }
}
