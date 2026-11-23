//! Transposition table for caching search results.
//!
//! Uses Zobrist hashes to store and retrieve position evaluations,
//! enabling significant search tree pruning.
//!
//! This implementation uses lockless hashing for thread-safe access
//! in multi-threaded (Lazy SMP) search. Entries are stored as atomic
//! u64 pairs using XOR verification to detect torn reads.

use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::board::Move;

mod entry;
mod slot;

pub use entry::{BoundType, TTEntry};

use entry::{pack_entry, MAX_HALFMOVE_CLOCK};
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
    buckets: Box<[TTBucket]>,
    mask: usize,
    ever_stored: AtomicBool,
}

impl TranspositionTable {
    /// Create a new transposition table with the given size in megabytes.
    #[must_use]
    pub fn new(size_mb: usize) -> Self {
        let bucket_size = mem::size_of::<TTBucket>();
        let num_buckets = tt_bucket_count(size_mb, bucket_size);

        TranspositionTable {
            buckets: Self::empty_buckets(num_buckets),
            mask: num_buckets - 1,
            ever_stored: AtomicBool::new(false),
        }
    }

    fn empty_buckets(num_buckets: usize) -> Box<[TTBucket]> {
        // SAFETY: A bucket contains only atomic integers and padding.
        // Zero is valid for both atomic fields and represents an empty slot.
        unsafe { Box::<[TTBucket]>::new_zeroed_slice(num_buckets).assume_init() }
    }

    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    /// Prefetch TT bucket for a hash to hide memory latency.
    /// Call this with the child hash before searching the child position.
    #[inline]
    pub fn prefetch(&self, hash: u64) {
        #[cfg(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64"))]
        let bucket_ptr = self.buckets.as_ptr().wrapping_add(self.index(hash));

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

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // PRFM is a cache hint and does not read shared atomic data.
            std::arch::asm!(
                "prfm pldl1keep, [{address}]",
                address = in(reg) bucket_ptr,
                options(readonly, nostack, preserves_flags),
            );
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
        let _ = hash;
    }

    /// Probe the table for an entry matching the given hash.
    /// Returns None if no valid entry is found.
    #[must_use]
    pub fn probe(&self, hash: u64) -> Option<TTEntry> {
        let bucket = &self.buckets[self.index(hash)];
        bucket.probe(hash)
    }

    /// Store an entry for a position with a zero halfmove clock.
    /// Use `store_with_halfmove_clock` when retaining fifty-move rule context.
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
        self.store_with_halfmove_clock(hash, depth, score, bound_type, best_move, generation, 0);
    }

    /// Store a score together with its fifty-move rule context.
    #[allow(clippy::too_many_arguments)]
    pub fn store_with_halfmove_clock(
        &self,
        hash: u64,
        depth: u32,
        score: i32,
        bound_type: BoundType,
        best_move: Option<Move>,
        generation: u16,
        halfmove_clock: u32,
    ) {
        // This flag only transitions from false to true. A sweep racing a
        // writer cannot reset it and accidentally skip a later clear.
        if !self.ever_stored.load(Ordering::Relaxed) {
            self.ever_stored.store(true, Ordering::Relaxed);
        }
        let depth_u8 = depth.min(255) as u8;
        let score_i16 = score.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        let gen_u8 = (generation & 0x3F) as u8;

        let packed = pack_entry(
            depth_u8,
            score_i16,
            bound_type,
            best_move,
            gen_u8,
            halfmove_clock.min(MAX_HALFMOVE_CLOCK) as u8,
        );
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
        if !self.ever_stored.load(Ordering::Relaxed) {
            return;
        }
        for bucket in &self.buckets {
            bucket.clear();
        }
    }

    pub(crate) fn clear_exclusive(&mut self) {
        if *self.ever_stored.get_mut() {
            let bucket_count = self.buckets.len();
            // Release the old allocation first to avoid holding two tables
            // in memory. The allocator can supply zeroed pages on demand.
            drop(mem::take(&mut self.buckets));
            self.buckets = Self::empty_buckets(bucket_count);
            *self.ever_stored.get_mut() = false;
        }
    }
}

#[cfg(test)]
mod tests;
