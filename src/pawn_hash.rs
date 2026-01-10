//! Pawn hash table for caching pawn structure evaluation.
//!
//! Pawn structure only depends on pawn positions, so it can be cached
//! using a pawn-only Zobrist hash. This provides significant speedup
//! since pawn structure evaluation is called frequently but pawns
//! rarely move.

use std::sync::atomic::{AtomicU64, Ordering};

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
}

impl PawnSlot {
    fn new() -> Self {
        PawnSlot {
            key_xor: AtomicU64::new(0),
            data: AtomicU64::new(0),
        }
    }

    fn store(&self, hash: u64, packed: u64) {
        self.data.store(packed, Ordering::Relaxed);
        self.key_xor.store(hash ^ packed, Ordering::Relaxed);
    }

    fn probe(&self, hash: u64) -> Option<PawnHashEntry> {
        let key_xor = self.key_xor.load(Ordering::Relaxed);
        let data = self.data.load(Ordering::Relaxed);

        // XOR verification detects torn reads
        if key_xor ^ data == hash && data != 0 {
            Some(unpack_entry(data))
        } else {
            None
        }
    }

    fn is_empty(&self) -> bool {
        self.data.load(Ordering::Relaxed) == 0
    }
}

/// Number of slots per bucket
const BUCKET_SIZE: usize = 2;

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
}

/// Thread-safe pawn hash table using lockless hashing.
///
/// Caches pawn structure evaluation results indexed by pawn-only Zobrist hash.
/// Can be safely shared across threads in SMP search.
pub struct PawnHashTable {
    buckets: Vec<PawnBucket>,
    mask: usize,
}

// Safety: PawnSlot uses AtomicU64 which is Send + Sync
unsafe impl Send for PawnHashTable {}
unsafe impl Sync for PawnHashTable {}

impl PawnHashTable {
    /// Create a new pawn hash table with the given size in kilobytes.
    /// Default is 1024 KB (1 MB).
    #[must_use]
    pub fn new(size_kb: usize) -> Self {
        let bucket_size = std::mem::size_of::<PawnBucket>();
        let mut num_buckets = (size_kb * 1024) / bucket_size;

        // Ensure power of 2 for efficient indexing
        num_buckets = num_buckets.next_power_of_two() / 2;
        if num_buckets == 0 {
            num_buckets = 1024;
        }

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
        let bucket = &self.buckets[self.index(pawn_hash)];
        for slot in &bucket.slots {
            if let Some(entry) = slot.probe(pawn_hash) {
                return Some(entry);
            }
        }
        None
    }

    /// Store pawn structure evaluation in the table.
    pub fn store(&self, pawn_hash: u64, mg: i32, eg: i32) {
        let packed = pack_entry(mg, eg);
        let bucket = &self.buckets[self.index(pawn_hash)];

        // First pass: look for empty slot or matching hash
        for slot in &bucket.slots {
            if slot.is_empty() || slot.probe(pawn_hash).is_some() {
                slot.store(pawn_hash, packed);
                return;
            }
        }

        // Replace first slot if no empty/matching slot found
        bucket.slots[0].store(pawn_hash, packed);
    }

    /// Clear all entries from the table.
    pub fn clear(&self) {
        for bucket in &self.buckets {
            for slot in &bucket.slots {
                slot.key_xor.store(0, Ordering::Relaxed);
                slot.data.store(0, Ordering::Relaxed);
            }
        }
    }
}

impl Default for PawnHashTable {
    fn default() -> Self {
        Self::new(1024) // 1 MB default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_roundtrip() {
        let test_cases = [(100, 200), (-500, 300), (0, 0), (i32::MAX, i32::MIN)];

        for (mg, eg) in test_cases {
            let packed = pack_entry(mg, eg);
            let unpacked = unpack_entry(packed);
            assert_eq!(unpacked.mg, mg);
            assert_eq!(unpacked.eg, eg);
        }
    }

    #[test]
    fn test_store_and_probe() {
        let table = PawnHashTable::new(64);
        let hash = 0x123456789ABCDEF0;

        table.store(hash, 150, -50);

        let entry = table.probe(hash).expect("should find entry");
        assert_eq!(entry.mg, 150);
        assert_eq!(entry.eg, -50);
    }

    #[test]
    fn test_no_false_positives() {
        let table = PawnHashTable::new(64);
        let hash1 = 0x123456789ABCDEF0;
        let hash2 = 0xFEDCBA9876543210;

        table.store(hash1, 100, 200);

        assert!(table.probe(hash2).is_none());
    }

    #[test]
    fn test_update_existing() {
        let table = PawnHashTable::new(64);
        let hash = 0x123456789ABCDEF0;

        table.store(hash, 100, 200);
        table.store(hash, 300, 400);

        let entry = table.probe(hash).expect("should find entry");
        assert_eq!(entry.mg, 300);
        assert_eq!(entry.eg, 400);
    }

    #[test]
    fn test_clear() {
        let table = PawnHashTable::new(64);
        let hash = 0x123456789ABCDEF0;

        table.store(hash, 100, 200);
        assert!(table.probe(hash).is_some());

        table.clear();
        assert!(table.probe(hash).is_none());
    }
}
