//! Transposition table for caching search results.
//!
//! Uses Zobrist hashes to store and retrieve position evaluations,
//! enabling significant search tree pruning.

use std::mem;

use crate::board::Move;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,      // Score is the exact value
    LowerBound, // Score is at least this value (failed low - score <= alpha)
    UpperBound, // Score is at most this value (failed high - score >= beta)
}

#[derive(Clone, Debug)]
pub(crate) struct TTEntry {
    hash: u64,
    depth: u32,
    score: i32,
    bound_type: BoundType,
    best_move: Option<Move>,
    generation: u16,
}

impl TTEntry {
    pub fn depth(&self) -> u32 {
        self.depth
    }

    pub fn score(&self) -> i32 {
        self.score
    }

    pub fn bound_type(&self) -> BoundType {
        self.bound_type
    }

    pub fn best_move(&self) -> Option<Move> {
        self.best_move
    }
}

pub struct TranspositionTable {
    table: Vec<[Option<TTEntry>; 4]>,
    mask: usize, // To wrap index around using bitwise AND (table size must be power of 2)
    occupied: usize,
}

impl TranspositionTable {
    // size_mb: Desired size in Megabytes
    #[must_use] 
    pub fn new(size_mb: usize) -> Self {
        let entry_size = mem::size_of::<[Option<TTEntry>; 4]>();
        let mut num_entries = (size_mb * 1024 * 1024) / entry_size;

        // Ensure num_entries is a power of 2 for efficient indexing
        num_entries = num_entries.next_power_of_two() / 2; // Find next power of 2, maybe go down one? Test this.
        if num_entries == 0 {
            num_entries = 1024;
        } // Minimum size fallback

        TranspositionTable {
            table: vec![[None, None, None, None]; num_entries],
            mask: num_entries - 1, // e.g., if size is 1024, mask is 1023 (0b1111111111)
            occupied: 0,
        }
    }

    // Calculate index using the hash and mask
    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    // Probe the table for a given hash
    pub(crate) fn probe(&self, hash: u64) -> Option<&TTEntry> {
        let index = self.index(hash);
        let bucket = &self.table[index];
        bucket.iter().flatten().find(|entry| entry.hash == hash)
    }

    // Store an entry in the table
    pub(crate) fn store(
        &mut self,
        hash: u64,
        depth: u32,
        score: i32,
        bound_type: BoundType,
        best_move: Option<Move>,
        generation: u16,
    ) {
        let index = self.index(hash);
        let bucket = &mut self.table[index];

        for slot in bucket.iter_mut() {
            if let Some(existing) = slot {
                if existing.hash == hash {
                    *slot = Some(TTEntry {
                        hash,
                        depth,
                        score,
                        bound_type,
                        best_move,
                        generation,
                    });
                    return;
                }
            }
        }

        for slot in bucket.iter_mut() {
            if slot.is_none() {
                *slot = Some(TTEntry {
                    hash,
                    depth,
                    score,
                    bound_type,
                    best_move,
                    generation,
                });
                self.occupied += 1;
                return;
            }
        }

        let mut replace_idx = 0;
        let mut worst_priority = i32::MAX;

        for (idx, slot) in bucket.iter().enumerate() {
            if let Some(entry) = slot {
                let age = generation.wrapping_sub(entry.generation);
                let priority = entry.depth.saturating_mul(2) as i32 - age as i32;
                if idx == 0 || priority < worst_priority {
                    replace_idx = idx;
                    worst_priority = priority;
                }
            }
        }

        bucket[replace_idx] = Some(TTEntry {
            hash,
            depth,
            score,
            bound_type,
            best_move,
            generation,
        });
    }

    #[must_use] 
    pub fn hashfull_per_mille(&self) -> u32 {
        let total_slots = self.table.len().saturating_mul(4);
        if total_slots == 0 {
            return 0;
        }
        ((self.occupied as u64 * 1000) / total_slots as u64) as u32
    }
}
