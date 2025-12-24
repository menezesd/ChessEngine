use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct RepetitionTable {
    counts: HashMap<u64, u32>,
}

impl RepetitionTable {
    pub(crate) fn new() -> Self {
        RepetitionTable {
            counts: HashMap::new(),
        }
    }

    pub(crate) fn get(&self, hash: u64) -> u32 {
        self.counts.get(&hash).copied().unwrap_or(0)
    }

    pub(crate) fn set(&mut self, hash: u64, count: u32) {
        if count == 0 {
            self.counts.remove(&hash);
        } else {
            self.counts.insert(hash, count);
        }
    }

    pub(crate) fn increment(&mut self, hash: u64) -> u32 {
        let next = self.get(hash).saturating_add(1);
        self.set(hash, next);
        next
    }
}
