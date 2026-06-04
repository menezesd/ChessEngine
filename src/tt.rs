//! Transposition table for caching search results.
//!
//! Uses Zobrist hashes to store and retrieve position evaluations,
//! enabling significant search tree pruning.
//!
//! This implementation uses lockless hashing for thread-safe access
//! in multi-threaded (Lazy SMP) search. Entries are stored as atomic
//! u64 pairs using XOR verification to detect torn reads.

use std::mem;

use crate::board::Move;

mod entry;
mod slot;

pub use entry::{BoundType, TTEntry};

use entry::pack_entry;
use slot::{TTBucket, BUCKET_SIZE};

const BYTES_PER_MIB: usize = 1024 * 1024;
const DEFAULT_BUCKETS: usize = 1024;

fn tt_bucket_count(size_mb: usize, bucket_size: usize) -> usize {
    crate::table_size::rounded_bucket_count(size_mb, BYTES_PER_MIB, bucket_size, DEFAULT_BUCKETS)
}

/// Thread-safe transposition table using lockless hashing.
///
/// Multiple threads can read and write concurrently without locks.
/// Torn reads are detected via XOR verification and discarded.
pub struct TranspositionTable {
    buckets: Vec<TTBucket>,
    mask: usize,
}

impl TranspositionTable {
    /// Create a new transposition table with the given size in megabytes.
    #[must_use]
    pub fn new(size_mb: usize) -> Self {
        let bucket_size = mem::size_of::<TTBucket>();
        let num_buckets = tt_bucket_count(size_mb, bucket_size);

        let mut buckets = Vec::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            buckets.push(TTBucket::new());
        }

        TranspositionTable {
            buckets,
            mask: num_buckets - 1,
        }
    }

    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    /// Prefetch TT bucket for a hash to hide memory latency.
    /// Call this before making a move, then probe after the move is made.
    #[inline]
    pub fn prefetch(&self, hash: u64) {
        let idx = self.index(hash);
        let bucket_ptr = self.buckets.as_ptr().wrapping_add(idx);

        // Use CPU prefetch instructions where available
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use std::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};
            _mm_prefetch(bucket_ptr.cast::<i8>(), _MM_HINT_T0);
        }

        #[cfg(target_arch = "x86")]
        unsafe {
            use std::arch::x86::{_mm_prefetch, _MM_HINT_T0};
            _mm_prefetch(bucket_ptr.cast::<i8>(), _MM_HINT_T0);
        }

        // Fallback for other architectures: volatile read brings cache line in
        #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
        unsafe {
            std::ptr::read_volatile(bucket_ptr.cast::<u8>());
        }
    }

    /// Probe the table for an entry matching the given hash.
    /// Returns None if no valid entry is found.
    #[must_use]
    pub fn probe(&self, hash: u64) -> Option<TTEntry> {
        let bucket = &self.buckets[self.index(hash)];
        bucket.probe(hash)
    }

    /// Store an entry in the table.
    ///
    /// Uses a replacement strategy that prefers:
    /// 1. Empty slots
    /// 2. Slots with matching hash (update)
    /// 3. Slots with lowest priority (old generation, shallow depth)
    pub fn store(
        &self,
        hash: u64,
        depth: u32,
        score: i32,
        bound_type: BoundType,
        best_move: Option<Move>,
        generation: u16,
    ) {
        let depth_u8 = depth.min(255) as u8;
        let score_i16 = score.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        let gen_u8 = (generation & 0x3F) as u8;

        let packed = pack_entry(depth_u8, score_i16, bound_type, best_move, gen_u8);
        let bucket = &self.buckets[self.index(hash)];
        bucket.store(hash, packed, gen_u8);
    }

    /// Returns hash table fullness in per mille (0-1000).
    #[must_use]
    pub fn hashfull_per_mille(&self) -> u32 {
        // Sample first 1000 buckets for efficiency
        let sample_size = self.buckets.len().min(1000);
        let mut occupied = 0;

        for bucket in self.buckets.iter().take(sample_size) {
            occupied += bucket.occupied_count();
        }

        let total_slots = sample_size * BUCKET_SIZE;
        ((occupied as u64 * 1000) / total_slots as u64) as u32
    }

    /// Clear all entries from the table.
    pub fn clear(&self) {
        for bucket in &self.buckets {
            bucket.clear();
        }
    }
}

#[cfg(test)]
mod tests;
