//! Core types for the chess engine

use std::collections::HashSet;

// --- Enums ---

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Color {
    White,
    Black,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Square(pub usize, pub usize); // (rank, file)

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub is_castling: bool,
    pub is_en_passant: bool,
    pub promotion: Option<Piece>,
    pub captured_piece: Option<Piece>,
}

#[derive(Clone, Debug)]
pub struct UnmakeInfo {
    pub captured_piece_info: Option<(Color, Piece)>,
    pub previous_en_passant_target: Option<Square>,
    pub previous_castling_rights: HashSet<(Color, char)>,
    pub previous_hash: u64,
}

// --- Transposition Table ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Debug)]
pub struct TTEntry {
    pub hash: u64,
    pub depth: u32,
    pub score: i32,
    pub bound_type: BoundType,
    pub best_move: Option<Move>,
}

pub struct TranspositionTable {
    pub table: Vec<Option<TTEntry>>,
    pub mask: usize,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        use std::mem;
        let entry_size = mem::size_of::<Option<TTEntry>>();
        let mut num_entries = (size_mb * 1024 * 1024) / entry_size;

        num_entries = num_entries.next_power_of_two() / 2;
        if num_entries == 0 {
            num_entries = 1024;
        }

        TranspositionTable {
            table: vec![None; num_entries],
            mask: num_entries - 1,
        }
    }

    pub fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    pub fn probe(&self, hash: u64) -> Option<&TTEntry> {
        let index = self.index(hash);
        if let Some(entry) = &self.table[index] {
            if entry.hash == hash {
                return Some(entry);
            }
        }
        None
    }

    pub fn store(
        &mut self,
        hash: u64,
        depth: u32,
        score: i32,
        bound_type: BoundType,
        best_move: Option<Move>,
    ) {
        let index = self.index(hash);
        let should_replace = match &self.table[index] {
            Some(existing_entry) => depth >= existing_entry.depth,
            None => true,
        };

        if should_replace {
            self.table[index] = Some(TTEntry {
                hash,
                depth,
                score,
                bound_type,
                best_move,
            });
        }
    }
}

// --- Constants ---

pub const PAWN_VALUE: i32 = 100;
pub const KNIGHT_VALUE: i32 = 320;
pub const BISHOP_VALUE: i32 = 330;
pub const ROOK_VALUE: i32 = 500;
pub const QUEEN_VALUE: i32 = 900;
pub const KING_VALUE: i32 = 20000;
pub const MATE_SCORE: i32 = KING_VALUE * 10;

// --- Helper Functions ---

pub fn file_to_index(file: char) -> usize {
    file as usize - ('a' as usize)
}

pub fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('0' as usize) - 1
}

pub fn format_square(sq: Square) -> String {
    format!("{}{}", (sq.1 as u8 + b'a') as char, sq.0 + 1)
}
