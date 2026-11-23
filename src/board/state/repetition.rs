use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// The map keys are already 64-bit Zobrist hashes.
#[derive(Default)]
struct PositionHasher(u64);

impl Hasher for PositionHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.write_u64(u64::from_ne_bytes(
            bytes.try_into().expect("position keys are u64"),
        ));
    }

    fn write_u64(&mut self, key: u64) {
        self.0 = key;
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RepetitionTable {
    counts: HashMap<u64, u32, BuildHasherDefault<PositionHasher>>,
}

impl RepetitionTable {
    pub(crate) fn new() -> Self {
        RepetitionTable {
            counts: HashMap::default(),
        }
    }

    pub(crate) fn get(&self, hash: u64) -> u32 {
        self.counts.get(&hash).copied().unwrap_or(0)
    }

    pub(crate) fn has_repeated_position(&self) -> bool {
        self.counts.values().any(|&count| count > 1)
    }

    pub(crate) fn set(&mut self, hash: u64, count: u32) {
        if count == 0 {
            self.counts.remove(&hash);
        } else {
            self.counts.insert(hash, count);
        }
    }

    /// Record the current position and return its old count for unmake.
    pub(crate) fn record(&mut self, hash: u64) -> u32 {
        let count = self.counts.entry(hash).or_insert(0);
        let previous = *count;
        *count = previous.saturating_add(1);
        previous
    }

    pub(crate) fn clear(&mut self) {
        self.counts.clear();
    }
}
