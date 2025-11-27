use std::collections::HashMap;

use crate::core::types::Piece;
use crate::core::board::Board;
use crate::core::zobrist::{ZOBRIST, piece_to_zobrist_index};

/// Stores pre-computed evaluation data for a given pawn structure hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PawnEntry {
    pub pmg: i32,
    pub peg: i32,
}

/// A simple pawn hash table that stores evaluation results for pawn structures.
pub struct PawnHashTable {
    table: HashMap<u64, PawnEntry>,
}

impl PawnHashTable {
    pub fn new() -> Self {
        PawnHashTable {
            table: HashMap::new(),
        }
    }

    /// Generates a Zobrist hash for only the pawn structure on the board.
    /// This hash is used as a key for the pawn hash table.
    pub fn generate_pawn_hash(board: &Board) -> u64 {
        let mut hash = 0u64;
        let pawn_idx = piece_to_zobrist_index(Piece::Pawn);

        for color_idx in 0..2 {
            let mut pawns = board.bitboards[color_idx][pawn_idx];
            while pawns != 0 {
                let sq_idx = pawns.trailing_zeros() as usize;
                hash ^= ZOBRIST.piece_keys[pawn_idx][color_idx][sq_idx];
                pawns &= pawns - 1;
            }
        }
        hash
    }

    /// Probes the pawn hash table for an entry.
    pub fn probe(&self, pawn_hash: u64) -> Option<PawnEntry> {
        self.table.get(&pawn_hash).copied()
    }

    /// Stores a pawn entry in the pawn hash table.
    pub fn store(&mut self, pawn_hash: u64, entry: PawnEntry) {
        self.table.insert(pawn_hash, entry);
    }
}