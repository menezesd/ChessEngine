use std::mem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,
    LowerBound,
    UpperBound,
}

use crate::Move;

#[derive(Clone, Debug)]
pub struct TTEntry {
    pub(crate) hash: u64,
    pub(crate) depth: u32,
    pub(crate) score: i32,
    pub(crate) bound_type: BoundType,
    pub(crate) best_move: Option<Move>,
}

pub struct TranspositionTable {
    table: Vec<Option<TTEntry>>,
    mask: usize,
}

impl TranspositionTable {
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
        let idx = self.index(hash);
        if let Some(entry) = &self.table[idx] {
            if entry.hash == hash {
                return Some(entry);
            }
        }
        None
    }

    pub(crate) fn store(
        &mut self,
        hash: u64,
        depth: u32,
        score: i32,
        bound_type: BoundType,
        best_move: Option<Move>,
    ) {
        let idx = self.index(hash);
        let should_replace = match &self.table[idx] {
            Some(e) => depth >= e.depth,
            None => true,
        };
        if should_replace {
            self.table[idx] = Some(TTEntry {
                hash,
                depth,
                score,
                bound_type,
                best_move,
            });
        }
    }

    pub fn resize(&mut self, size_mb: usize) {
        let entry_size = mem::size_of::<Option<TTEntry>>();
        let mut num_entries = (size_mb * 1024 * 1024) / entry_size;
        num_entries = num_entries.next_power_of_two() / 2;
        if num_entries == 0 {
            num_entries = 1024;
        }
        self.table = vec![None; num_entries];
        self.mask = num_entries - 1;
    }

    pub fn clear(&mut self) {
        for slot in self.table.iter_mut() {
            *slot = None;
        }
    }
}
