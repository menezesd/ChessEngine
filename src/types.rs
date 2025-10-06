use std::collections::HashSet;
use std::mem; // For size_of

// Material values
pub const PAWN_VALUE: i32 = 100;
pub const KNIGHT_VALUE: i32 = 320;
pub const BISHOP_VALUE: i32 = 330;
pub const ROOK_VALUE: i32 = 500;
pub const QUEEN_VALUE: i32 = 900;
pub const KING_VALUE: i32 = 20000;
pub const MATE_SCORE: i32 = KING_VALUE * 10;

/// Helper functions
pub fn file_to_index(file: char) -> usize {
    file as usize - ('a' as usize)
}

pub fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('0' as usize) - 1
}

/// Piece types
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

/// Colors
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Color {
    White,
    Black,
}

/// Board square
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Square(pub usize, pub usize);

/// Chess move
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub is_castling: bool,
    pub is_en_passant: bool,
    pub promotion: Option<Piece>,
    pub captured_piece: Option<Piece>,
}

/// Information to unmake a move
#[derive(Clone, Debug)]
pub struct UnmakeInfo {
    pub captured_piece_info: Option<(Color, Piece)>,
    pub previous_en_passant_target: Option<Square>,
    pub previous_castling_rights: HashSet<(Color, char)>,
    pub previous_hash: u64,
}

/// Transposition Table bound type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,
    LowerBound,
    UpperBound,
}

/// Transposition Table entry
#[derive(Clone, Debug)]
pub struct TTEntry {
    pub hash: u64,
    pub depth: u32,
    pub score: i32,
    pub bound_type: BoundType,
    pub best_move: Option<Move>,
}

/// Transposition Table
pub struct TranspositionTable {
    pub table: Vec<Option<TTEntry>>,
    pub mask: usize,
}

impl TranspositionTable {
    /// Create a new table of size in MB
    pub fn new(size_mb: usize) -> Self {
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

    /// Index into table
    pub fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    /// Probe entry by hash
    pub fn probe(&self, hash: u64) -> Option<&TTEntry> {
        let idx = self.index(hash);
        if let Some(entry) = &self.table[idx] {
            if entry.hash == hash {
                return Some(entry);
            }
        }
        None
    }

    /// Store an entry
    pub fn store(&mut self, hash: u64, depth: u32, score: i32, bound_type: BoundType, best_move: Option<Move>) {
        let idx = self.index(hash);
        self.table[idx] = Some(TTEntry { hash, depth, score, bound_type, best_move });
    }

    /// Clear table
    pub fn clear(&mut self) {
        for slot in &mut self.table {
            *slot = None;
        }
    }
}
