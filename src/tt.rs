//! Transposition table for caching search results.
//!
//! Uses Zobrist hashes to store and retrieve position evaluations,
//! enabling significant search tree pruning.
//!
//! This implementation uses lockless hashing for thread-safe access
//! in multi-threaded (Lazy SMP) search. Entries are stored as atomic
//! u64 pairs using XOR verification to detect torn reads.

use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::board::Move;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,      // Score is the exact value
    LowerBound, // Score is at least this value (failed high - score >= beta)
    UpperBound, // Score is at most this value (failed low - score <= alpha)
}

impl BoundType {
    fn to_u8(self) -> u8 {
        match self {
            BoundType::Exact => 0,
            BoundType::LowerBound => 1,
            BoundType::UpperBound => 2,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v & 0x3 {
            0 => BoundType::Exact,
            1 => BoundType::LowerBound,
            _ => BoundType::UpperBound,
        }
    }
}

/// Unpacked TT entry for reading
#[derive(Clone, Debug)]
pub struct TTEntry {
    pub depth: u8,
    pub score: i16,
    pub bound_type: BoundType,
    pub best_move: Option<Move>,
    pub generation: u8,
}

impl TTEntry {
    pub fn depth(&self) -> u32 {
        self.depth as u32
    }

    pub fn score(&self) -> i32 {
        self.score as i32
    }

    pub fn bound_type(&self) -> BoundType {
        self.bound_type
    }

    pub fn best_move(&self) -> Option<Move> {
        self.best_move
    }
}

/// Packed entry format (fits in 64 bits):
/// - bits 0-15:  move (u16, 0 = no move)
/// - bits 16-31: score (i16 as u16)
/// - bits 32-39: depth (u8)
/// - bits 40-47: bound (2 bits) + generation (6 bits)
///
/// Total: 48 bits used, 16 bits spare
fn pack_entry(
    depth: u8,
    score: i16,
    bound_type: BoundType,
    best_move: Option<Move>,
    generation: u8,
) -> u64 {
    let mv: u16 = best_move.map(|m| m.as_u16()).unwrap_or(0);
    let sc: u16 = score as u16;
    let bound_gen: u8 = (bound_type.to_u8() & 0x3) | ((generation & 0x3F) << 2);

    (mv as u64)
        | ((sc as u64) << 16)
        | ((depth as u64) << 32)
        | ((bound_gen as u64) << 40)
}

fn unpack_entry(data: u64) -> TTEntry {
    let mv_bits = (data & 0xFFFF) as u16;
    let score = ((data >> 16) & 0xFFFF) as i16;
    let depth = ((data >> 32) & 0xFF) as u8;
    let bound_gen = ((data >> 40) & 0xFF) as u8;

    let bound_type = BoundType::from_u8(bound_gen & 0x3);
    let generation = (bound_gen >> 2) & 0x3F;

    let best_move = if mv_bits == 0 {
        None
    } else {
        Some(Move::from_u16(mv_bits))
    };

    TTEntry {
        depth,
        score,
        bound_type,
        best_move,
        generation,
    }
}

/// A single TT slot using lockless hashing.
///
/// Uses the XOR technique: stores (key ^ data) and data separately.
/// On read, we verify by checking if (stored_key ^ data) equals the probe key.
/// This detects torn reads from concurrent writes.
#[repr(C)]
struct TTSlot {
    /// Stores: hash_key ^ packed_data
    key_xor: AtomicU64,
    /// Stores: packed_data
    data: AtomicU64,
}

impl TTSlot {
    fn new() -> Self {
        TTSlot {
            key_xor: AtomicU64::new(0),
            data: AtomicU64::new(0),
        }
    }

    fn store(&self, hash: u64, packed: u64) {
        // Write data first, then key_xor
        // This order ensures that if we read a valid key_xor,
        // the data is already written
        self.data.store(packed, Ordering::Relaxed);
        self.key_xor.store(hash ^ packed, Ordering::Relaxed);
    }

    fn probe(&self, hash: u64) -> Option<TTEntry> {
        // Read in opposite order from store
        let key_xor = self.key_xor.load(Ordering::Relaxed);
        let data = self.data.load(Ordering::Relaxed);

        // Verify: key_xor ^ data should equal the hash we're probing
        if key_xor ^ data == hash && data != 0 {
            Some(unpack_entry(data))
        } else {
            None
        }
    }

    fn is_empty(&self) -> bool {
        self.data.load(Ordering::Relaxed) == 0
    }

    fn generation(&self) -> u8 {
        let data = self.data.load(Ordering::Relaxed);
        if data == 0 {
            0
        } else {
            (((data >> 40) & 0xFF) as u8 >> 2) & 0x3F
        }
    }

    fn depth(&self) -> u8 {
        let data = self.data.load(Ordering::Relaxed);
        ((data >> 32) & 0xFF) as u8
    }
}

/// Number of slots per bucket for collision resolution
const BUCKET_SIZE: usize = 4;

/// A bucket containing multiple slots
#[repr(C)]
struct TTBucket {
    slots: [TTSlot; BUCKET_SIZE],
}

impl TTBucket {
    fn new() -> Self {
        TTBucket {
            slots: [TTSlot::new(), TTSlot::new(), TTSlot::new(), TTSlot::new()],
        }
    }
}

/// Thread-safe transposition table using lockless hashing.
///
/// Multiple threads can read and write concurrently without locks.
/// Torn reads are detected via XOR verification and discarded.
pub struct TranspositionTable {
    buckets: Vec<TTBucket>,
    mask: usize,
}

// Safety: TTSlot uses AtomicU64 which is Send + Sync
unsafe impl Send for TranspositionTable {}
unsafe impl Sync for TranspositionTable {}

impl TranspositionTable {
    /// Create a new transposition table with the given size in megabytes.
    #[must_use]
    pub fn new(size_mb: usize) -> Self {
        let bucket_size = mem::size_of::<TTBucket>();
        let mut num_buckets = (size_mb * 1024 * 1024) / bucket_size;

        // Ensure num_buckets is a power of 2 for efficient indexing
        num_buckets = num_buckets.next_power_of_two() / 2;
        if num_buckets == 0 {
            num_buckets = 1024;
        }

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

    /// Probe the table for an entry matching the given hash.
    /// Returns None if no valid entry is found.
    pub fn probe(&self, hash: u64) -> Option<TTEntry> {
        let bucket = &self.buckets[self.index(hash)];
        for slot in &bucket.slots {
            if let Some(entry) = slot.probe(hash) {
                return Some(entry);
            }
        }
        None
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

        // First pass: look for empty slot or matching hash
        for slot in &bucket.slots {
            if slot.is_empty() {
                slot.store(hash, packed);
                return;
            }
            // Check if this slot has the same position (update it)
            if let Some(_) = slot.probe(hash) {
                slot.store(hash, packed);
                return;
            }
        }

        // Second pass: find worst slot to replace
        let mut replace_idx = 0;
        let mut worst_priority = i32::MAX;

        for (idx, slot) in bucket.slots.iter().enumerate() {
            let slot_gen = slot.generation();
            let slot_depth = slot.depth();
            let age = gen_u8.wrapping_sub(slot_gen) & 0x3F;
            let priority = (slot_depth as i32) * 2 - (age as i32);

            if priority < worst_priority {
                replace_idx = idx;
                worst_priority = priority;
            }
        }

        bucket.slots[replace_idx].store(hash, packed);
    }

    /// Returns hash table fullness in per mille (0-1000).
    #[must_use]
    pub fn hashfull_per_mille(&self) -> u32 {
        // Sample first 1000 buckets for efficiency
        let sample_size = self.buckets.len().min(1000);
        let mut occupied = 0;

        for bucket in self.buckets.iter().take(sample_size) {
            for slot in &bucket.slots {
                if !slot.is_empty() {
                    occupied += 1;
                }
            }
        }

        let total_slots = sample_size * BUCKET_SIZE;
        ((occupied as u64 * 1000) / total_slots as u64) as u32
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_roundtrip() {
        let test_cases = [
            (10u8, 500i16, BoundType::Exact, Some(Move::from_u16(0x1234)), 5u8),
            (255u8, -32000i16, BoundType::LowerBound, None, 63u8),
            (0u8, 0i16, BoundType::UpperBound, Some(Move::from_u16(0xFFFF)), 0u8),
        ];

        for (depth, score, bound, mv, gen) in test_cases {
            let packed = pack_entry(depth, score, bound, mv, gen);
            let unpacked = unpack_entry(packed);

            assert_eq!(unpacked.depth, depth);
            assert_eq!(unpacked.score, score);
            assert_eq!(unpacked.bound_type, bound);
            assert_eq!(unpacked.best_move.map(|m| m.as_u16()), mv.map(|m| m.as_u16()));
            assert_eq!(unpacked.generation, gen);
        }
    }

    #[test]
    fn test_store_and_probe() {
        let tt = TranspositionTable::new(1);
        let hash = 0x123456789ABCDEF0;

        tt.store(hash, 10, 500, BoundType::Exact, None, 1);

        let entry = tt.probe(hash).expect("should find entry");
        assert_eq!(entry.depth, 10);
        assert_eq!(entry.score, 500);
        assert_eq!(entry.bound_type, BoundType::Exact);
    }

    #[test]
    fn test_no_false_positives() {
        let tt = TranspositionTable::new(1);
        let hash1 = 0x123456789ABCDEF0;
        let hash2 = 0xFEDCBA9876543210;

        tt.store(hash1, 10, 500, BoundType::Exact, None, 1);

        assert!(tt.probe(hash2).is_none());
    }
}
