//! Pawn hash table for caching pawn structure evaluation.
//!
//! Pawn structure only depends on pawn positions, so it can be cached
//! using a pawn-only Zobrist hash. This provides significant speedup
//! since pawn structure evaluation is called frequently but pawns
//! rarely move.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::table_size::rounded_bucket_count;

/// Entry returned from pawn hash table probe
#[derive(Clone, Copy, Debug)]
pub struct PawnHashEntry {
    pub mg: i32,
    pub eg: i32,
}

/// Pack mg and eg scores into a single u64
/// Format: mg (i32) in lower 32 bits, eg (i32) in upper 32 bits
#[inline]
fn pack_entry(mg: i32, eg: i32) -> u64 {
    let mg_bits = mg as u32;
    let eg_bits = eg as u32;
    (mg_bits as u64) | ((eg_bits as u64) << 32)
}

/// Unpack mg and eg scores from a u64
#[inline]
fn unpack_entry(data: u64) -> PawnHashEntry {
    let mg = data as u32 as i32;
    let eg = (data >> 32) as u32 as i32;
    PawnHashEntry { mg, eg }
}

/// A single slot in the pawn hash table using lockless hashing.
/// Uses XOR technique for thread-safety without locks.
#[repr(C)]
struct PawnSlot {
    /// Stores: `pawn_hash` ^ `packed_data`
    key_xor: AtomicU64,
    /// Stores: `packed_data`
    data: AtomicU64,
    /// Distinguishes an empty slot from a valid `(hash, mg, eg) == (0, 0, 0)` entry.
    occupied: AtomicBool,
}

impl PawnSlot {
    fn new() -> Self {
        PawnSlot {
            key_xor: AtomicU64::new(0),
            data: AtomicU64::new(0),
            occupied: AtomicBool::new(false),
        }
    }

    fn store(&self, hash: u64, packed: u64) {
        self.data.store(packed, Ordering::Relaxed);
        self.key_xor.store(hash ^ packed, Ordering::Relaxed);
        self.occupied.store(true, Ordering::Release);
    }

    fn probe(&self, hash: u64) -> Option<PawnHashEntry> {
        if !self.occupied.load(Ordering::Acquire) {
            return None;
        }

        let key_xor = self.key_xor.load(Ordering::Relaxed);
        let data = self.data.load(Ordering::Relaxed);

        // XOR verification detects torn reads
        if key_xor ^ data == hash {
            Some(unpack_entry(data))
        } else {
            None
        }
    }

    fn is_empty(&self) -> bool {
        !self.occupied.load(Ordering::Acquire)
    }

    fn clear(&self) {
        self.occupied.store(false, Ordering::Release);
        self.key_xor.store(0, Ordering::Relaxed);
        self.data.store(0, Ordering::Relaxed);
    }
}

/// Number of slots per bucket
const BUCKET_SIZE: usize = 2;
const BYTES_PER_KIB: usize = 1024;
const DEFAULT_BUCKETS: usize = 1024;

fn pawn_bucket_count(size_kb: usize, bucket_size: usize) -> usize {
    rounded_bucket_count(size_kb, BYTES_PER_KIB, bucket_size, DEFAULT_BUCKETS)
}

/// A bucket containing multiple slots for collision resolution
#[repr(C)]
struct PawnBucket {
    slots: [PawnSlot; BUCKET_SIZE],
}

impl PawnBucket {
    fn new() -> Self {
        PawnBucket {
            slots: [PawnSlot::new(), PawnSlot::new()],
        }
    }

    fn probe(&self, hash: u64) -> Option<PawnHashEntry> {
        self.slots.iter().find_map(|slot| slot.probe(hash))
    }

    fn available_slot(&self, hash: u64) -> Option<&PawnSlot> {
        self.slots
            .iter()
            .find(|slot| slot.is_empty() || slot.probe(hash).is_some())
    }

    fn store(&self, hash: u64, packed: u64) {
        let slot = self.available_slot(hash).unwrap_or(&self.slots[0]);
        slot.store(hash, packed);
    }

    fn clear(&self) {
        for slot in &self.slots {
            slot.clear();
        }
    }
}

/// Thread-safe pawn hash table using lockless hashing.
///
/// Caches pawn structure evaluation results indexed by pawn-only Zobrist hash.
/// Can be safely shared across threads in SMP search.
pub struct PawnHashTable {
    buckets: Vec<PawnBucket>,
    mask: usize,
}

impl PawnHashTable {
    /// Create a new pawn hash table with the given size in kilobytes.
    /// Default is 1024 KB (1 MB).
    #[must_use]
    pub fn new(size_kb: usize) -> Self {
        let bucket_size = std::mem::size_of::<PawnBucket>();
        let num_buckets = pawn_bucket_count(size_kb, bucket_size);

        let mut buckets = Vec::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            buckets.push(PawnBucket::new());
        }

        PawnHashTable {
            buckets,
            mask: num_buckets - 1,
        }
    }

    #[inline]
    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    /// Probe the table for cached pawn structure evaluation.
    #[must_use]
    pub fn probe(&self, pawn_hash: u64) -> Option<PawnHashEntry> {
        self.buckets[self.index(pawn_hash)].probe(pawn_hash)
    }

    /// Store pawn structure evaluation in the table.
    pub fn store(&self, pawn_hash: u64, mg: i32, eg: i32) {
        let packed = pack_entry(mg, eg);
        self.buckets[self.index(pawn_hash)].store(pawn_hash, packed);
    }

    /// Clear all entries from the table.
    pub fn clear(&self) {
        for bucket in &self.buckets {
            bucket.clear();
        }
    }
}

impl Default for PawnHashTable {
    fn default() -> Self {
        Self::new(1024) // 1 MB default
    }
}

#[cfg(test)]
mod tests;
