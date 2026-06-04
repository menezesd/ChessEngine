use std::sync::atomic::{AtomicU64, Ordering};

use super::entry::{
    unpack_entry, BOUND_GEN_SHIFT, BYTE_MASK, DEPTH_SHIFT, GENERATION_MASK, GENERATION_SHIFT,
};
use super::TTEntry;

/// A single TT slot using lockless hashing.
///
/// Uses the XOR technique: stores `(key ^ data)` and data separately.
/// On read, we verify by checking if `(stored_key ^ data)` equals the probe key.
/// This detects torn reads from concurrent writes.
#[repr(C)]
pub(super) struct TTSlot {
    /// Stores: `hash_key ^ packed_data`.
    key_xor: AtomicU64,
    /// Stores: `packed_data`.
    data: AtomicU64,
}

impl TTSlot {
    fn new() -> Self {
        TTSlot {
            key_xor: AtomicU64::new(0),
            data: AtomicU64::new(0),
        }
    }

    pub(super) fn store(&self, hash: u64, packed: u64) {
        self.data.store(packed, Ordering::Relaxed);
        self.key_xor.store(hash ^ packed, Ordering::Relaxed);
    }

    pub(super) fn probe(&self, hash: u64) -> Option<TTEntry> {
        let key_xor = self.key_xor.load(Ordering::Relaxed);
        let data = self.data.load(Ordering::Relaxed);

        if key_xor ^ data == hash && data != 0 {
            Some(unpack_entry(data))
        } else {
            None
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.data.load(Ordering::Relaxed) == 0
    }

    pub(super) fn generation(&self) -> u8 {
        let data = self.data.load(Ordering::Relaxed);
        if data == 0 {
            0
        } else {
            let bound_gen = ((data >> BOUND_GEN_SHIFT) & BYTE_MASK) as u8;
            (bound_gen >> GENERATION_SHIFT) & GENERATION_MASK
        }
    }

    pub(super) fn depth(&self) -> u8 {
        let data = self.data.load(Ordering::Relaxed);
        ((data >> DEPTH_SHIFT) & BYTE_MASK) as u8
    }

    pub(super) fn clear(&self) {
        self.key_xor.store(0, Ordering::Relaxed);
        self.data.store(0, Ordering::Relaxed);
    }
}

/// Number of slots per bucket for collision resolution.
pub(super) const BUCKET_SIZE: usize = 4;

/// A bucket containing multiple slots.
#[repr(C)]
pub(super) struct TTBucket {
    pub(super) slots: [TTSlot; BUCKET_SIZE],
}

impl TTBucket {
    pub(super) fn new() -> Self {
        TTBucket {
            slots: [TTSlot::new(), TTSlot::new(), TTSlot::new(), TTSlot::new()],
        }
    }

    pub(super) fn probe(&self, hash: u64) -> Option<TTEntry> {
        for slot in &self.slots {
            if let Some(entry) = slot.probe(hash) {
                return Some(entry);
            }
        }
        None
    }

    pub(super) fn store(&self, hash: u64, packed: u64, generation: u8) {
        let slot = self
            .available_slot(hash)
            .unwrap_or_else(|| self.replacement_slot(generation));
        slot.store(hash, packed);
    }

    pub(super) fn occupied_count(&self) -> usize {
        self.slots.iter().filter(|slot| !slot.is_empty()).count()
    }

    pub(super) fn clear(&self) {
        for slot in &self.slots {
            slot.clear();
        }
    }

    fn available_slot(&self, hash: u64) -> Option<&TTSlot> {
        self.slots
            .iter()
            .find(|slot| slot.is_empty() || slot.probe(hash).is_some())
    }

    fn replacement_slot(&self, generation: u8) -> &TTSlot {
        self.slots
            .iter()
            .min_by_key(|slot| replacement_priority(slot, generation))
            .expect("TT bucket has at least one slot")
    }
}

fn replacement_priority(slot: &TTSlot, generation: u8) -> i32 {
    let age = generation.wrapping_sub(slot.generation()) & 0x3F;
    (slot.depth() as i32) * 2 - (age as i32)
}
