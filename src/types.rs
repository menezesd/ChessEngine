// --- Enums and Structs ---

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
    pub previous_castling_rights: u8,
    pub previous_hash: u64, // Store previous hash for unmake
}

#[derive(Clone, Debug)]
pub struct NullInfo {
    pub previous_en_passant_target: Option<Square>,
    pub previous_castling_rights: u8,
    pub previous_hash: u64,
}

// --- Transposition Table ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,      // Score is the exact value
    LowerBound, // Score is at least this value (failed low - score <= alpha)
    UpperBound, // Score is at most this value (failed high - score >= beta)
}

#[derive(Clone, Debug)] // Move needs to be Clone for TTEntry
pub struct TTEntry {
    pub hash: u64,               // Store the full hash to verify entry
    pub depth: u32,              // Depth of the search that stored this entry
    pub score: i32,              // The score found
    pub bound_type: BoundType,   // Type of score (Exact, LowerBound, UpperBound)
    pub best_move: Option<Move>, // Best move found from this position (for move ordering)
}

pub struct TranspositionTable {
    pub table: Vec<Option<TTEntry>>,
    pub mask: usize, // To wrap index around using bitwise AND (table size must be power of 2)
}

impl TranspositionTable {
    // size_mb: Desired size in Megabytes
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<Option<TTEntry>>();
        let mut num_entries = (size_mb * 1024 * 1024) / entry_size;

        // Ensure num_entries is a power of 2 for efficient indexing
        num_entries = num_entries.next_power_of_two() / 2; // Find next power of 2, maybe go down one? Test this.
        if num_entries == 0 {
            num_entries = 1024;
        } // Minimum size fallback

        TranspositionTable {
            table: vec![None; num_entries],
            mask: num_entries - 1, // e.g., if size is 1024, mask is 1023 (0b1111111111)
        }
    }

    // Calculate index using the hash and mask
    pub fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    // Probe the table for a given hash
    pub fn probe(&self, hash: u64) -> Option<&TTEntry> {
        let index = self.index(hash);
        if let Some(entry) = &self.table[index] {
            if entry.hash == hash {
                // Verify hash to handle collisions
                return Some(entry);
            }
        }
        None
    }

    // Store an entry in the table
    // Uses simple always-replace scheme
    pub fn store(
        &mut self,
        hash: u64,
        depth: u32,
        score: i32,
        bound_type: BoundType,
        best_move: Option<Move>,
    ) {
        let index = self.index(hash);
        // Store only if the new entry is from a deeper search or replaces nothing
        // (Could implement more sophisticated replacement strategies)
        let should_replace = match &self.table[index] {
            Some(existing_entry) => depth >= existing_entry.depth, // Replace if new search is deeper or equal
            None => true,                                          // Replace if slot is empty
        };

        if should_replace {
            self.table[index] = Some(TTEntry {
                hash,
                depth,
                score,
                bound_type,
                best_move, // Pass ownership/clone
            });
        }
    }

    // Clear the entire transposition table
    pub fn clear(&mut self) {
        for entry in &mut self.table {
            *entry = None;
        }
    }
}

// --- Utility Functions ---

pub fn format_square(sq: Square) -> String {
    format!(
        "{}{}",
        (b'a' + sq.1 as u8) as char,
        (b'1' + sq.0 as u8) as char
    )
}

pub fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('1' as usize)
}

pub fn file_to_index(file: char) -> usize {
    (file as usize) - ('a' as usize)
}
