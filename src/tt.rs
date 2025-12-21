use std::mem;

use crate::board::Move;

// --- Transposition Table ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoundType {
    Exact,      // Score is the exact value
    LowerBound, // Score is at least this value (failed low - score <= alpha)
    UpperBound, // Score is at most this value (failed high - score >= beta)
}

#[derive(Clone, Debug)] // Move needs to be Clone for TTEntry
pub(crate) struct TTEntry {
    pub(crate) hash: u64,               // Store the full hash to verify entry
    pub(crate) depth: u32,              // Depth of the search that stored this entry
    pub(crate) score: i32,              // The score found
    pub(crate) bound_type: BoundType,   // Type of score (Exact, LowerBound, UpperBound)
    pub(crate) best_move: Option<Move>, // Best move found from this position (for move ordering)
}

pub struct TranspositionTable {
    table: Vec<Option<TTEntry>>,
    mask: usize, // To wrap index around using bitwise AND (table size must be power of 2)
}

impl TranspositionTable {
    // size_mb: Desired size in Megabytes
    pub fn new(size_mb: usize) -> Self {
        let entry_size = mem::size_of::<Option<TTEntry>>();
        let mut num_entries = (size_mb * 1024 * 1024) / entry_size;

        // Ensure num_entries is a power of 2 for efficient indexing
        num_entries = num_entries.next_power_of_two() / 2; // Find next power of 2, maybe go down one? Test this.
        if num_entries == 0 {
            num_entries = 1024;
        } // Minimum size fallback

        TranspositionTable {
            table: vec![None; num_entries],
            mask: num_entries - 1, // e.g., if size is 1024, mask is 1023 (0b1111111111)
        }
    }

    // Calculate index using the hash and mask
    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    // Probe the table for a given hash
    pub(crate) fn probe(&self, hash: u64) -> Option<&TTEntry> {
        let index = self.index(hash);
        if let Some(entry) = &self.table[index] {
            if entry.hash == hash {
                // Verify hash to handle collisions
                return Some(entry);
            }
        }
        None
    }

    // Store an entry in the table
    // Uses simple always-replace scheme
    pub(crate) fn store(
        &mut self,
        hash: u64,
        depth: u32,
        score: i32,
        bound_type: BoundType,
        best_move: Option<Move>,
    ) {
        let index = self.index(hash);
        // Store only if the new entry is from a deeper search or replaces nothing
        // (Could implement more sophisticated replacement strategies)
        let should_replace = match &self.table[index] {
            Some(existing_entry) => depth >= existing_entry.depth, // Replace if new search is deeper or equal
            None => true,                                          // Replace if slot is empty
        };

        if should_replace {
            self.table[index] = Some(TTEntry {
                hash,
                depth,
                score,
                bound_type,
                best_move, // Pass ownership/clone
            });
        }
    }
}
