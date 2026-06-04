use crate::board::{Move, EMPTY_MOVE, MAX_PLY};

const KILLER_SLOTS: usize = 3;
const PRIMARY_SLOT: usize = 0;
const SECONDARY_SLOT: usize = 1;
const TERTIARY_SLOT: usize = 2;

pub struct KillerTable {
    slots: [[Move; KILLER_SLOTS]; MAX_PLY],
}

impl Default for KillerTable {
    fn default() -> Self {
        Self::new()
    }
}

impl KillerTable {
    #[must_use]
    pub fn new() -> Self {
        KillerTable {
            slots: [[EMPTY_MOVE; KILLER_SLOTS]; MAX_PLY],
        }
    }

    /// Get killer move at given ply and slot (0=primary, 1=secondary, 2=tertiary)
    #[must_use]
    pub fn get(&self, ply: usize, slot: usize) -> Move {
        self.slots
            .get(ply)
            .and_then(|row| row.get(slot))
            .copied()
            .unwrap_or(EMPTY_MOVE)
    }

    #[must_use]
    #[inline]
    pub fn primary(&self, ply: usize) -> Move {
        self.get(ply, PRIMARY_SLOT)
    }

    #[must_use]
    #[inline]
    pub fn secondary(&self, ply: usize) -> Move {
        self.get(ply, SECONDARY_SLOT)
    }

    #[must_use]
    #[inline]
    pub fn tertiary(&self, ply: usize) -> Move {
        self.get(ply, TERTIARY_SLOT)
    }

    pub fn update(&mut self, ply: usize, mv: Move) {
        if ply >= MAX_PLY {
            return;
        }
        if self.slots[ply][PRIMARY_SLOT] != mv {
            self.slots[ply][TERTIARY_SLOT] = self.slots[ply][SECONDARY_SLOT];
            self.slots[ply][SECONDARY_SLOT] = self.slots[ply][PRIMARY_SLOT];
            self.slots[ply][PRIMARY_SLOT] = mv;
        }
    }

    pub fn reset(&mut self) {
        for killers in &mut self.slots {
            killers.fill(EMPTY_MOVE);
        }
    }
}
