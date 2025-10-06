use std::collections::HashSet;
use std::io;
use std::io::{BufRead, Write};
use std::mem; // For size_of
use std::time::{Duration, Instant};
// Module declarations for split code
mod types;
mod zobrist;
mod utils;
mod board;
mod perft;

// --- Crates ---
use once_cell::sync::Lazy;
use rand::prelude::*;

// --- Zobrist Hashing ---

// Struct to hold all Zobrist keys
struct ZobristKeys {
    // piece_keys[piece_type][color][square_index]
    piece_keys: [[[u64; 64]; 2]; 6], // PieceType(0-5), Color(0-1), Square(0-63)
    black_to_move_key: u64,
    // castling_keys[color][side] : 0=White, 1=Black; 0=Kingside, 1=Queenside
    castling_keys: [[u64; 2]; 2],
    // en_passant_keys[file_index] (only file matters for EP target)
    en_passant_keys: [u64; 8],
}

impl ZobristKeys {
    fn new() -> Self {
        let mut rng = StdRng::seed_from_u64(1234567890_u64); // Use a fixed seed for reproducibility
        let mut piece_keys = [[[0; 64]; 2]; 6];
        let mut castling_keys = [[0; 2]; 2];
        let mut en_passant_keys = [0; 8];

        for p_idx in 0..6 {
            // Piece type
            for c_idx in 0..2 {
                // Color
                for sq_idx in 0..64 {
                    // Square
                    piece_keys[p_idx][c_idx][sq_idx] = rng.gen();
                }
            }
        }

        let black_to_move_key = rng.gen();

        for c_idx in 0..2 {
            // Color
            for side_idx in 0..2 {
                // Side (K=0, Q=1)
                castling_keys[c_idx][side_idx] = rng.gen();
            }
        }

        for f_idx in 0..8 {
            // File
            en_passant_keys[f_idx] = rng.gen();
        }

        ZobristKeys {
            piece_keys,
            black_to_move_key,
            castling_keys,
            en_passant_keys,
        }
    }
}

// Initialize Zobrist keys lazily and globally
static ZOBRIST: Lazy<ZobristKeys> = Lazy::new(ZobristKeys::new);

// Helper to map Piece enum to index
fn piece_to_zobrist_index(piece: Piece) -> usize {
    match piece {
        Piece::Pawn => 0,
        Piece::Knight => 1,
        Piece::Bishop => 2,
        Piece::Rook => 3,
        Piece::Queen => 4,
        Piece::King => 5,
    }
}

// Helper to map Color enum to index
fn color_to_zobrist_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}

// Helper to map Square to index (0-63)
fn square_to_zobrist_index(sq: Square) -> usize {
    sq.0 * 8 + sq.1
}

// Material values (unchanged)
const PAWN_VALUE: i32 = 100;
const KNIGHT_VALUE: i32 = 320;
const BISHOP_VALUE: i32 = 330;
const ROOK_VALUE: i32 = 500;
const QUEEN_VALUE: i32 = 900;
const KING_VALUE: i32 = 20000;
const MATE_SCORE: i32 = KING_VALUE * 10;

// --- Helper functions (unchanged) ---
fn file_to_index(file: char) -> usize {
    file as usize - ('a' as usize)
}

fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('0' as usize) - 1
}

// --- Enums and Structs ---

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Color {
    White,
    Black,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Square(usize, usize); // (rank, file)

#[derive(Clone, Copy, Debug, PartialEq)]
struct Move {
    from: Square,
    to: Square,
    is_castling: bool,
    is_en_passant: bool,
    promotion: Option<Piece>,
    captured_piece: Option<Piece>,
}

#[derive(Clone, Debug)]
struct UnmakeInfo {
    captured_piece_info: Option<(Color, Piece)>,
    previous_en_passant_target: Option<Square>,
    previous_castling_rights: HashSet<(Color, char)>,
    previous_hash: u64, // Store previous hash for unmake
}

// --- Transposition Table ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundType {
    Exact,      // Score is the exact value
    LowerBound, // Score is at least this value (failed low - score <= alpha)
    UpperBound, // Score is at most this value (failed high - score >= beta)
}

#[derive(Clone, Debug)] // Move needs to be Clone for TTEntry
struct TTEntry {
    hash: u64,               // Store the full hash to verify entry
    depth: u32,              // Depth of the search that stored this entry
    score: i32,              // The score found
    bound_type: BoundType,   // Type of score (Exact, LowerBound, UpperBound)
    best_move: Option<Move>, // Best move found from this position (for move ordering)
}

struct TranspositionTable {
    table: Vec<Option<TTEntry>>,
    mask: usize, // To wrap index around using bitwise AND (table size must be power of 2)
}

impl TranspositionTable {
    // size_mb: Desired size in Megabytes
    fn new(size_mb: usize) -> Self {
        let entry_size = mem::size_of::<Option<TTEntry>>();
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
    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    // Probe the table for a given hash
    fn probe(&self, hash: u64) -> Option<&TTEntry> {
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
    fn store(
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
}

fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 300,
        Piece::Bishop => 300,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 10000, // Usually not used in MVV-LVA since king captures are illegal
    }
}

fn mvv_lva_score(m: &Move, board: &Board) -> i32 {
    if let Some(victim) = m.captured_piece {
        let attacker = board.squares[m.from.0][m.from.1].unwrap().1;
        let victim_value = piece_value(victim);
        let attacker_value = piece_value(attacker);
        victim_value * 10 - attacker_value // prioritize more valuable victims, less valuable attackers
    } else {
        0 // Non-captures get low priority
    }
}

#[derive(Clone, Debug)]
struct Board {
    squares: [[Option<(Color, Piece)>; 8]; 8],
    white_to_move: bool,
    en_passant_target: Option<Square>,
    castling_rights: HashSet<(Color, char)>, // 'K' or 'Q'
    hash: u64,                               // Add Zobrist hash field
}

impl Board {
    fn new() -> Self {
        // ... (initialization of squares, rights etc. as before) ...
        let mut squares = [[None; 8]; 8];
        let back_rank = [
            Piece::Rook,
            Piece::Knight,
            Piece::Bishop,
            Piece::Queen,
            Piece::King,
            Piece::Bishop,
            Piece::Knight,
            Piece::Rook,
        ];
        for (i, piece) in back_rank.iter().enumerate() {
            squares[0][i] = Some((Color::White, *piece));
            squares[7][i] = Some((Color::Black, *piece));
            squares[1][i] = Some((Color::White, Piece::Pawn));
            squares[6][i] = Some((Color::Black, Piece::Pawn));
        }
        let mut castling_rights = HashSet::new();
        castling_rights.insert((Color::White, 'K'));
        castling_rights.insert((Color::White, 'Q'));
        castling_rights.insert((Color::Black, 'K'));
        castling_rights.insert((Color::Black, 'Q'));

        let mut board = Board {
            squares,
            white_to_move: true,
            en_passant_target: None,
            castling_rights,
            hash: 0, // Initialize hash to 0 before calculating
        };
        board.hash = board.calculate_initial_hash(); // Calculate hash after setting up board
        board
    }

    fn from_fen(fen: &str) -> Self {
        // ... (parsing FEN as before) ...
        let mut squares = [[None; 8]; 8];
        let mut castling_rights = HashSet::new();
        let parts: Vec<&str> = fen.split_whitespace().collect();
        assert!(parts.len() >= 4, "FEN must have at least 4 parts");
        for (rank_idx, rank_str) in parts[0].split('/').enumerate() {
            let mut file = 0;
            for c in rank_str.chars() {
                if c.is_digit(10) {
                    file += c.to_digit(10).unwrap() as usize;
                } else {
                    let (color, piece) = match c {
                        'P' => (Color::White, Piece::Pawn),
                        'N' => (Color::White, Piece::Knight),
                        'B' => (Color::White, Piece::Bishop),
                        'R' => (Color::White, Piece::Rook),
                        'Q' => (Color::White, Piece::Queen),
                        'K' => (Color::White, Piece::King),
                        'p' => (Color::Black, Piece::Pawn),
                        'n' => (Color::Black, Piece::Knight),
                        'b' => (Color::Black, Piece::Bishop),
                        'r' => (Color::Black, Piece::Rook),
                        'q' => (Color::Black, Piece::Queen),
                        'k' => (Color::Black, Piece::King),
                        _ => panic!("Invalid piece char"),
                    };
                    squares[7 - rank_idx][file] = Some((color, piece));
                    file += 1;
                }
            }
        }
        let white_to_move = match parts[1] {
            "w" => true,
            "b" => false,
            _ => panic!("Invalid color"),
        };
        for c in parts[2].chars() {
            match c {
                'K' => {
                    castling_rights.insert((Color::White, 'K'));
                }
                'Q' => {
                    castling_rights.insert((Color::White, 'Q'));
                }
                'k' => {
                    castling_rights.insert((Color::Black, 'K'));
                }
                'q' => {
                    castling_rights.insert((Color::Black, 'Q'));
                }
                '-' => {}
                _ => panic!("Invalid castle"),
            }
        }
        let en_passant_target = if parts[3] != "-" {
            let chars: Vec<char> = parts[3].chars().collect();
            if chars.len() == 2 {
                Some(Square(rank_to_index(chars[1]), file_to_index(chars[0])))
            } else {
                None
            }
        } else {
            None
        };

        let mut board = Board {
            squares,
            white_to_move,
            en_passant_target,
            castling_rights,
            hash: 0, // Initialize hash to 0 before calculating
        };
        board.hash = board.calculate_initial_hash(); // Calculate hash after setting up board
        board
    }

    // Calculate Zobrist hash from scratch
    fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;

        // Pieces
        for r in 0..8 {
            for f in 0..8 {
                if let Some((color, piece)) = self.squares[r][f] {
                    let sq_idx = square_to_zobrist_index(Square(r, f));
                    let p_idx = piece_to_zobrist_index(piece);
                    let c_idx = color_to_zobrist_index(color);
                    hash ^= ZOBRIST.piece_keys[p_idx][c_idx][sq_idx];
                }
            }
        }

        // Side to move
        if !self.white_to_move {
            hash ^= ZOBRIST.black_to_move_key;
        }

        // Castling rights
        if self.castling_rights.contains(&(Color::White, 'K')) {
            hash ^= ZOBRIST.castling_keys[0][0];
        }
        if self.castling_rights.contains(&(Color::White, 'Q')) {
            hash ^= ZOBRIST.castling_keys[0][1];
        }
        if self.castling_rights.contains(&(Color::Black, 'K')) {
            hash ^= ZOBRIST.castling_keys[1][0];
        }
        if self.castling_rights.contains(&(Color::Black, 'Q')) {
            hash ^= ZOBRIST.castling_keys[1][1];
        }

        // En passant target
        if let Some(ep_square) = self.en_passant_target {
            hash ^= ZOBRIST.en_passant_keys[ep_square.1]; // XOR based on the file
        }

        hash
    }

    // --- Make/Unmake Logic ---

    fn make_move(&mut self, m: &Move) -> UnmakeInfo {
        let mut current_hash = self.hash;
        let previous_hash = self.hash; // Store hash before changes

        let color = self.current_color();

        // Store previous state for unmaking
        let previous_en_passant_target = self.en_passant_target;
        let previous_castling_rights = self.castling_rights.clone();

        // 1. Update hash: Side to move always changes
        current_hash ^= ZOBRIST.black_to_move_key;

        // 2. Update hash: Remove old en passant target file (if any)
        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= ZOBRIST.en_passant_keys[old_ep.1];
        }

        // Determine captured piece *before* moving
        let mut captured_piece_info: Option<(Color, Piece)> = None;
        let mut captured_sq_idx: Option<usize> = None;

        if m.is_en_passant {
            let capture_row = if color == Color::White {
                m.to.0 - 1
            } else {
                m.to.0 + 1
            };
            let capture_sq = Square(capture_row, m.to.1);
            captured_sq_idx = Some(square_to_zobrist_index(capture_sq));
            captured_piece_info = self.squares[capture_row][m.to.1]; // Should be Some(opp_color, Pawn)
            self.squares[capture_row][m.to.1] = None; // Remove the en passant captured pawn

            // Update hash: remove captured EP pawn
            if let Some((cap_col, cap_piece)) = captured_piece_info {
                current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                    [color_to_zobrist_index(cap_col)][captured_sq_idx.unwrap()];
            }
        } else if !m.is_castling {
            // Regular capture or move to empty square
            captured_piece_info = self.squares[m.to.0][m.to.1];
            if captured_piece_info.is_some() {
                captured_sq_idx = Some(square_to_zobrist_index(m.to));
                // Update hash: remove captured piece from target square
                if let Some((cap_col, cap_piece)) = captured_piece_info {
                    current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                        [color_to_zobrist_index(cap_col)][captured_sq_idx.unwrap()];
                }
            }
        }
        // No capture hash update needed for castling here

        // Get the piece being moved
        let moving_piece_info = self.squares[m.from.0][m.from.1].expect("make_move 'from' empty");
        let (moving_color, moving_piece) = moving_piece_info;
        let from_sq_idx = square_to_zobrist_index(m.from);
        let to_sq_idx = square_to_zobrist_index(m.to);

        // Update hash: Remove moving piece from 'from' square
        current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(moving_piece)]
            [color_to_zobrist_index(moving_color)][from_sq_idx];

        // Update board squares
        self.squares[m.from.0][m.from.1] = None;

        if m.is_castling {
            // Handle King and Rook moves for castling
            self.squares[m.to.0][m.to.1] = Some((color, Piece::King)); // Place King
                                                                       // Update hash: Place king on 'to' square
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::King)]
                [color_to_zobrist_index(color)][to_sq_idx];

            let (rook_from_f, rook_to_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) }; // KS or QS
            let rook_from_sq = Square(m.to.0, rook_from_f);
            let rook_to_sq = Square(m.to.0, rook_to_f);
            let rook_info =
                self.squares[rook_from_sq.0][rook_from_sq.1].expect("Castling without rook");
            self.squares[rook_from_sq.0][rook_from_sq.1] = None;
            self.squares[rook_to_sq.0][rook_to_sq.1] = Some(rook_info);

            // Update hash: Remove rook from original square
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)]
                [color_to_zobrist_index(color)][square_to_zobrist_index(rook_from_sq)];
            // Update hash: Place rook on its destination square
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)]
                [color_to_zobrist_index(color)][square_to_zobrist_index(rook_to_sq)];
        } else {
            // Regular move, promotion, or en passant landing
            let piece_to_place = if let Some(promoted_piece) = m.promotion {
                (color, promoted_piece)
            } else {
                moving_piece_info
            };
            self.squares[m.to.0][m.to.1] = Some(piece_to_place);
            // Update hash: Place the (potentially promoted) piece on 'to' square
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(piece_to_place.1)]
                [color_to_zobrist_index(piece_to_place.0)][to_sq_idx];
        }

        // Update en_passant_target & hash
        self.en_passant_target = None; // Clear by default
        if moving_piece == Piece::Pawn && (m.from.0 as isize - m.to.0 as isize).abs() == 2 {
            let ep_row = (m.from.0 + m.to.0) / 2;
            let ep_sq = Square(ep_row, m.from.1);
            self.en_passant_target = Some(ep_sq);
            // Update hash: add new en passant target file
            current_hash ^= ZOBRIST.en_passant_keys[ep_sq.1];
        }

        // --- Update castling rights & hash ---
        let mut new_castling_rights = self.castling_rights.clone();
        let mut castle_hash_diff: u64 = 0; // XOR changes here

        // Check rights lost due to piece movement
        if moving_piece == Piece::King {
            if self.castling_rights.contains(&(color, 'K')) {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                new_castling_rights.remove(&(color, 'K'));
            }
            if self.castling_rights.contains(&(color, 'Q')) {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                new_castling_rights.remove(&(color, 'Q'));
            }
        } else if moving_piece == Piece::Rook {
            let start_rank = if color == Color::White { 0 } else { 7 };
            if m.from == Square(start_rank, 0) && self.castling_rights.contains(&(color, 'Q')) {
                // A-side rook moved
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                new_castling_rights.remove(&(color, 'Q'));
            } else if m.from == Square(start_rank, 7)
                && self.castling_rights.contains(&(color, 'K'))
            {
                // H-side rook moved
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                new_castling_rights.remove(&(color, 'K'));
            }
        }

        // Check rights lost due to capture OF a rook on its starting square
        if let Some((captured_color, captured_piece)) = captured_piece_info {
            if captured_piece == Piece::Rook {
                let start_rank = if captured_color == Color::White { 0 } else { 7 };
                if m.to == Square(start_rank, 0)
                    && self.castling_rights.contains(&(captured_color, 'Q'))
                {
                    // A-side rook captured
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][1];
                    new_castling_rights.remove(&(captured_color, 'Q'));
                } else if m.to == Square(start_rank, 7)
                    && self.castling_rights.contains(&(captured_color, 'K'))
                {
                    // H-side rook captured
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][0];
                    new_castling_rights.remove(&(captured_color, 'K'));
                }
            }
        }
        self.castling_rights = new_castling_rights;
        current_hash ^= castle_hash_diff; // Apply castling hash changes

        // Switch turn
        self.white_to_move = !self.white_to_move;

        // Update the board's hash
        self.hash = current_hash;

        // Return unmake info
        UnmakeInfo {
            captured_piece_info,
            previous_en_passant_target,
            previous_castling_rights,
            previous_hash, // Pass the old hash back
        }
    }

    // Unmake move now restores the hash directly
    fn unmake_move(&mut self, m: &Move, info: UnmakeInfo) {
        // Restore state directly from info
        self.white_to_move = !self.white_to_move; // Switch turn back first
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash; // Restore hash directly!

        // Restore pieces on board (no hash updates needed here as hash is fully restored)
        let color = self.current_color();

        let piece_that_moved = if m.promotion.is_some() {
            (color, Piece::Pawn)
        } else if m.is_castling {
            (color, Piece::King) // Assume king if castling
        } else {
            self.squares[m.to.0][m.to.1].expect("Unmake move: 'to' square empty?")
        };

        if m.is_castling {
            self.squares[m.from.0][m.from.1] = Some(piece_that_moved); // Put King back
            self.squares[m.to.0][m.to.1] = None;

            let (rook_orig_f, rook_moved_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) }; // KS or QS
            let rook_info =
                self.squares[m.to.0][rook_moved_f].expect("Unmake castling: rook missing");
            self.squares[m.to.0][rook_moved_f] = None;
            self.squares[m.to.0][rook_orig_f] = Some(rook_info); // Put Rook back
        } else {
            self.squares[m.from.0][m.from.1] = Some(piece_that_moved); // Move piece back

            if m.is_en_passant {
                self.squares[m.to.0][m.to.1] = None; // Clear landing square
                let capture_row = if color == Color::White {
                    m.to.0 - 1
                } else {
                    m.to.0 + 1
                };
                self.squares[capture_row][m.to.1] = info.captured_piece_info; // Restore EP captured pawn
            } else {
                // Regular move: Put back captured piece (or None)
                self.squares[m.to.0][m.to.1] = info.captured_piece_info;
            }
        }
    }

    // --- Move Generation (largely unchanged logic, but uses new Move struct) ---
    // Note: Most generate_* functions remain &self, as they only read the board
    // generate_moves() will become &mut self because it uses make/unmake

    fn generate_pseudo_moves(&self) -> Vec<Move> {
        // ... implementation remains the same conceptually ...
        // ... but needs to populate `captured_piece` in the Move struct ...
        // This requires looking at the target square *before* creating the move.

        let mut moves = Vec::new();
        let color = if self.white_to_move {
            Color::White
        } else {
            Color::Black
        };

        for rank in 0..8 {
            for file in 0..8 {
                if let Some((c, piece)) = self.squares[rank][file] {
                    if c == color {
                        let from = Square(rank, file);
                        moves.extend(self.generate_piece_moves(from, piece));
                    }
                }
            }
        }
        moves
    }

    fn generate_piece_moves(&self, from: Square, piece: Piece) -> Vec<Move> {
        match piece {
            Piece::Pawn => self.generate_pawn_moves(from),
            Piece::Knight => self.generate_knight_moves(from),
            Piece::Bishop => {
                self.generate_sliding_moves(from, &[(1, 1), (1, -1), (-1, 1), (-1, -1)])
            }
            Piece::Rook => self.generate_sliding_moves(from, &[(1, 0), (-1, 0), (0, 1), (0, -1)]),
            Piece::Queen => self.generate_sliding_moves(
                from,
                &[
                    (1, 0),
                    (-1, 0),
                    (0, 1),
                    (0, -1),
                    (1, 1),
                    (1, -1),
                    (-1, 1),
                    (-1, -1),
                ],
            ),
            Piece::King => self.generate_king_moves(from),
        }
    }

    // Helper to create a move, checking for captures
    fn create_move(
        &self,
        from: Square,
        to: Square,
        promotion: Option<Piece>,
        is_castling: bool,
        is_en_passant: bool,
    ) -> Move {
        let captured_piece = if is_en_passant {
            // Color doesn't matter here, just the piece type
            Some(Piece::Pawn)
        } else if !is_castling {
            self.squares[to.0][to.1].map(|(_, p)| p)
        } else {
            None
        };

        Move {
            from,
            to,
            promotion,
            is_castling,
            is_en_passant,
            captured_piece,
        }
    }

    // Modify generation functions to use `create_move`
    fn generate_pawn_moves(&self, from: Square) -> Vec<Move> {
        let color = if self.white_to_move {
            Color::White
        } else {
            Color::Black
        };
        let mut moves = Vec::new();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let start_rank = if color == Color::White { 1 } else { 6 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        let r = from.0 as isize;
        let f = from.1 as isize;

        // Forward move
        let forward_r = r + dir;
        if forward_r >= 0 && forward_r < 8 {
            let forward_sq = Square(forward_r as usize, f as usize);
            if self.squares[forward_sq.0][forward_sq.1].is_none() {
                if forward_sq.0 == promotion_rank {
                    for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                        moves.push(self.create_move(from, forward_sq, Some(promo), false, false));
                    }
                } else {
                    moves.push(self.create_move(from, forward_sq, None, false, false));
                    // Double forward from starting rank
                    if r == start_rank as isize {
                        let double_forward_r = r + 2 * dir;
                        let double_forward_sq = Square(double_forward_r as usize, f as usize);
                        if self.squares[double_forward_sq.0][double_forward_sq.1].is_none() {
                            moves.push(self.create_move(
                                from,
                                double_forward_sq,
                                None,
                                false,
                                false,
                            ));
                        }
                    }
                }
            }
        }

        // Captures
        if forward_r >= 0 && forward_r < 8 {
            for df in [-1, 1] {
                let capture_f = f + df;
                if capture_f >= 0 && capture_f < 8 {
                    let target_sq = Square(forward_r as usize, capture_f as usize);
                    // Regular capture
                    if let Some((target_color, _)) = self.squares[target_sq.0][target_sq.1] {
                        if target_color != color {
                            if target_sq.0 == promotion_rank {
                                for promo in
                                    [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
                                {
                                    moves.push(self.create_move(
                                        from,
                                        target_sq,
                                        Some(promo),
                                        false,
                                        false,
                                    ));
                                }
                            } else {
                                moves.push(self.create_move(from, target_sq, None, false, false));
                            }
                        }
                    }
                    // En passant capture
                    else if Some(target_sq) == self.en_passant_target {
                        moves.push(self.create_move(from, target_sq, None, false, true));
                    }
                }
            }
        }

        moves
    }

    fn generate_knight_moves(&self, from: Square) -> Vec<Move> {
        let mut moves = Vec::new();
        let deltas = [
            (2, 1),
            (1, 2),
            (-1, 2),
            (-2, 1),
            (-2, -1),
            (-1, -2),
            (1, -2),
            (2, -1),
        ];
        let (rank, file) = (from.0 as isize, from.1 as isize);
        let color = self.current_color();

        for (dr, df) in deltas {
            let (nr, nf) = (rank + dr, file + df);
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                let to_sq = Square(nr as usize, nf as usize);
                if let Some((c, _)) = self.squares[to_sq.0][to_sq.1] {
                    if c != color {
                        moves.push(self.create_move(from, to_sq, None, false, false));
                    }
                } else {
                    moves.push(self.create_move(from, to_sq, None, false, false));
                }
            }
        }
        moves
    }

    fn generate_sliding_moves(&self, from: Square, directions: &[(isize, isize)]) -> Vec<Move> {
        let mut moves = Vec::new();
        let (rank, file) = (from.0 as isize, from.1 as isize);
        let color = self.current_color();

        for &(dr, df) in directions {
            let mut r = rank + dr;
            let mut f = file + df;
            while r >= 0 && r < 8 && f >= 0 && f < 8 {
                let to_sq = Square(r as usize, f as usize);
                if let Some((c, _)) = self.squares[to_sq.0][to_sq.1] {
                    if c != color {
                        // Capture
                        moves.push(self.create_move(from, to_sq, None, false, false));
                    }
                    break; // Stop searching in this direction (blocked)
                } else {
                    // Empty square
                    moves.push(self.create_move(from, to_sq, None, false, false));
                }
                r += dr;
                f += df;
            }
        }
        moves
    }

    fn generate_king_moves(&self, from: Square) -> Vec<Move> {
        let mut moves = Vec::new();
        let deltas = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        let (rank, file) = (from.0, from.1);
        let color = self.current_color();
        let back_rank = if color == Color::White { 0 } else { 7 };

        // Normal King moves
        for (dr, df) in deltas {
            let (nr, nf) = (rank as isize + dr, file as isize + df);
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                let to_sq = Square(nr as usize, nf as usize);
                if let Some((c, _)) = self.squares[to_sq.0][to_sq.1] {
                    if c != color {
                        moves.push(self.create_move(from, to_sq, None, false, false));
                    }
                } else {
                    moves.push(self.create_move(from, to_sq, None, false, false));
                }
            }
        }

        // Castling (only check conditions, legality check is separate)
        // Check if king is on its home rank and original square first
        if from == Square(back_rank, 4) {
            // Kingside
            if self.castling_rights.contains(&(color, 'K'))
                && self.squares[back_rank][5].is_none()
                && self.squares[back_rank][6].is_none()
                // Rook must be present
                && self.squares[back_rank][7] == Some((color, Piece::Rook))
            // Squares cannot be attacked (checked later in generate_moves)
            {
                let to_sq = Square(back_rank, 6);
                moves.push(self.create_move(from, to_sq, None, true, false));
            }
            // Queenside
            if self.castling_rights.contains(&(color, 'Q'))
                 && self.squares[back_rank][1].is_none()
                 && self.squares[back_rank][2].is_none()
                 && self.squares[back_rank][3].is_none()
                 // Rook must be present
                 && self.squares[back_rank][0] == Some((color, Piece::Rook))
            // Squares cannot be attacked (checked later in generate_moves)
            {
                let to_sq = Square(back_rank, 2);
                moves.push(self.create_move(from, to_sq, None, true, false));
            }
        }

        moves
    }

    // --- Check Detection (Refactored) ---

    // Finds the king of the specified color
    fn find_king(&self, color: Color) -> Option<Square> {
        for r in 0..8 {
            for f in 0..8 {
                if self.squares[r][f] == Some((color, Piece::King)) {
                    return Some(Square(r, f));
                }
            }
        }
        None
    }

    // Checks if a square is attacked by the opponent WITHOUT cloning
    // Takes &self because it only reads the state
    fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        let target_r = square.0 as isize;
        let target_f = square.1 as isize;

        // 1. Check for Pawn attacks
        let pawn_dir: isize = if attacker_color == Color::White {
            1
        } else {
            -1
        };
        let pawn_start_r = target_r - pawn_dir; // Where an attacking pawn would be
        if pawn_start_r >= 0 && pawn_start_r < 8 {
            for df in [-1, 1] {
                let pawn_start_f = target_f + df;
                if pawn_start_f >= 0 && pawn_start_f < 8 {
                    if self.squares[pawn_start_r as usize][pawn_start_f as usize]
                        == Some((attacker_color, Piece::Pawn))
                    {
                        return true;
                    }
                }
            }
        }

        // 2. Check for Knight attacks
        let knight_deltas = [
            (2, 1),
            (1, 2),
            (-1, 2),
            (-2, 1),
            (-2, -1),
            (-1, -2),
            (1, -2),
            (2, -1),
        ];
        for (dr, df) in knight_deltas {
            let r = target_r + dr;
            let f = target_f + df;
            if r >= 0 && r < 8 && f >= 0 && f < 8 {
                if self.squares[r as usize][f as usize] == Some((attacker_color, Piece::Knight)) {
                    return true;
                }
            }
        }

        // 3. Check for King attacks (for castling checks mainly, kings can't attack kings directly otherwise)
        let king_deltas = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        for (dr, df) in king_deltas {
            let r = target_r + dr;
            let f = target_f + df;
            if r >= 0 && r < 8 && f >= 0 && f < 8 {
                if self.squares[r as usize][f as usize] == Some((attacker_color, Piece::King)) {
                    return true;
                }
            }
        }

        // 4. Check for Sliding piece attacks (Rook, Bishop, Queen)
        let sliding_directions = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1), // Rook/Queen directions
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1), // Bishop/Queen directions
        ];

        for (i, &(dr, df)) in sliding_directions.iter().enumerate() {
            let is_diagonal = i >= 4;
            let mut r = target_r + dr;
            let mut f = target_f + df;

            while r >= 0 && r < 8 && f >= 0 && f < 8 {
                if let Some((piece_color, piece)) = self.squares[r as usize][f as usize] {
                    if piece_color == attacker_color {
                        // Is it the correct type of slider?
                        let can_attack = match piece {
                            Piece::Queen => true,
                            Piece::Rook => !is_diagonal,
                            Piece::Bishop => is_diagonal,
                            _ => false, // Pawn, Knight, King handled already or can't attack this way
                        };
                        if can_attack {
                            return true;
                        }
                    }
                    // Any piece (own or other) blocks further sliding checks in this direction
                    break;
                }
                r += dr;
                f += df;
            }
        }

        // No attackers found
        false
    }

    // Now takes &self
    fn is_in_check(&self, color: Color) -> bool {
        if let Some(king_sq) = self.find_king(color) {
            self.is_square_attacked(king_sq, self.opponent_color(color))
        } else {
            false // Or panic? King should always be on the board in a valid game.
        }
    }

    // Generates only fully legal moves, takes &mut self
    fn generate_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color();
        let opponent_color = self.opponent_color(current_color);
        let pseudo_moves = self.generate_pseudo_moves(); // Still generates based on current state
        let mut legal_moves = Vec::new();

        for m in pseudo_moves {
            // Special check for castling legality (squares king passes over cannot be attacked)
            if m.is_castling {
                let king_start_sq = m.from;
                let king_mid_sq = Square(m.from.0, (m.from.1 + m.to.1) / 2); // e.g., f1 or d1
                let king_end_sq = m.to;

                if self.is_square_attacked(king_start_sq, opponent_color)
                    || self.is_square_attacked(king_mid_sq, opponent_color)
                    || self.is_square_attacked(king_end_sq, opponent_color)
                {
                    continue; // Illegal castling move
                }
            }

            // Check general legality: Does the move leave the king in check?
            let info = self.make_move(&m); // Make the move temporarily
            if !self.is_in_check(current_color) {
                // Check if the player who moved is now safe
                legal_moves.push(m.clone()); // If safe, it's a legal move
            }
            self.unmake_move(&m, info); // Unmake the move to restore state for next iteration
        }
        legal_moves
    }

    // --- Game State Checks (need &mut self if they use generate_moves) ---

    // is_checkmate and is_stalemate now need &mut self
    fn is_checkmate(&mut self) -> bool {
        let color = self.current_color();
        self.is_in_check(color) && self.generate_moves().is_empty()
    }

    fn is_stalemate(&mut self) -> bool {
        let color = self.current_color();
        !self.is_in_check(color) && self.generate_moves().is_empty()
    }

    fn evaluate(&self) -> i32 {
        let mut score = 0;

        // Material values for middlegame and endgame
        const MATERIAL_MG: [i32; 6] = [82, 337, 365, 477, 1025, 20000]; // P, N, B, R, Q, K
        const MATERIAL_EG: [i32; 6] = [94, 281, 297, 512, 936, 20000]; // P, N, B, R, Q, K

        // Piece-square tables (middlegame)
        const PST_MG: [[i32; 64]; 6] = [
            // Pawn
            [
                0, 0, 0, 0, 0, 0, 0, 0, -35, -1, -20, -23, -15, 24, 38, -22, -26, -4, -4, -10, 3,
                3, 33, -12, -27, -2, -5, 12, 17, 6, 10, -25, -14, 13, 6, 21, 23, 12, 17, -23, -6,
                7, 26, 31, 65, 56, 25, -20, 98, 134, 61, 95, 68, 126, 34, -11, 0, 0, 0, 0, 0, 0, 0,
                0,
            ],
            // Knight
            [
                -105, -21, -58, -33, -17, -28, -19, -23, -29, -53, -12, -3, -1, 18, -14, -19, -23,
                -9, 12, 10, 19, 17, 25, -16, -13, 4, 16, 13, 28, 19, 21, -8, -9, 17, 19, 53, 37,
                69, 18, 22, -47, 60, 37, 65, 84, 129, 73, 44, -73, -41, 72, 36, 23, 62, 7, -17,
                -167, -89, -34, -49, 61, -97, -15, -107,
            ],
            // Bishop
            [
                -33, -3, -14, -21, -13, -12, -39, -21, 4, 15, 16, 0, 7, 21, 33, 1, 0, 15, 15, 15,
                14, 27, 18, 10, -6, 13, 13, 26, 34, 12, 10, 4, -4, 5, 19, 50, 37, 37, 7, -2, -16,
                37, 43, 40, 35, 50, 37, -2, -26, 16, -18, -13, 30, 59, 18, -47, -29, 4, -82, -37,
                -25, -42, 7, -8,
            ],
            // Rook
            [
                -19, -13, 1, 17, 16, 7, -37, -26, -44, -16, -20, -9, -1, 11, -6, -71, -45, -25,
                -16, -17, 3, 0, -5, -33, -36, -26, -12, -1, 9, -7, 6, -23, -24, -11, 7, 26, 24, 35,
                -8, -20, -5, 19, 26, 36, 17, 45, 61, 16, 27, 32, 58, 62, 80, 67, 26, 44, 32, 42,
                32, 51, 63, 9, 31, 43,
            ],
            // Queen
            [
                -1, -18, -9, 10, -15, -25, -31, -50, -35, -8, 11, 2, 8, 15, -3, 1, -14, 2, -11, -2,
                -5, 2, 14, 5, -9, -26, -9, -10, -2, -4, 3, -3, -27, -27, -16, -16, -1, 17, -2, 1,
                -13, -17, 7, 8, 29, 56, 47, 57, -24, -39, -5, 1, -16, 57, 28, 54, -28, 0, 29, 12,
                59, 44, 43, 45,
            ],
            // King
            [
                -15, 36, 12, -54, 8, -28, 34, 14, 1, 7, -8, -64, -43, -16, 9, 8, -14, -14, -22,
                -46, -44, -30, -15, -27, -49, -1, -27, -39, -46, -44, -33, -51, -17, -20, -12, -27,
                -30, -25, -14, -36, -9, 24, 2, -16, -20, 6, 22, -22, 29, -1, -20, -7, -8, -4, -38,
                -29, -65, 23, 16, -15, -56, -34, 2, 13,
            ],
        ];

        // Piece-square tables (endgame)
        const PST_EG: [[i32; 64]; 6] = [
            // Pawn
            [
                0, 0, 0, 0, 0, 0, 0, 0, 13, 8, 8, 10, 13, 0, 2, -7, 4, 7, -6, 1, 0, -5, -1, -8, 13,
                9, -3, -7, -7, -8, 3, -1, 32, 24, 13, 5, -2, 4, 17, 17, 94, 100, 85, 67, 56, 53,
                82, 84, 178, 173, 158, 134, 147, 132, 165, 187, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
            // Knight
            [
                -29, -51, -23, -15, -22, -18, -50, -64, -42, -20, -10, -5, -2, -20, -23, -44, -23,
                -3, -1, 15, 10, -3, -20, -22, -18, -6, 16, 25, 16, 17, 4, -18, -17, 3, 22, 22, 22,
                11, 8, -18, -24, -20, 10, 9, -1, -9, -19, -41, -25, -8, -25, -2, -9, -25, -24, -52,
                -58, -38, -13, -28, -31, -27, -63, -99,
            ],
            // Bishop
            [
                -23, -9, -23, -5, -9, -16, -5, -17, -14, -18, -7, -1, 4, -9, -15, -27, -12, -3, 8,
                10, 13, 3, -7, -15, -6, 3, 13, 19, 7, 10, -3, -9, -3, 9, 12, 9, 14, 10, 3, 2, 2,
                -8, 0, -1, -2, 6, 0, 4, -8, -4, 7, -12, -3, -13, -4, -14, -14, -21, -11, -8, -7,
                -9, -17, -24,
            ],
            // Rook
            [
                -9, 2, 3, -1, -5, -13, 4, -20, -6, -6, 0, 2, -9, -9, -11, -3, -4, 0, -5, -1, -7,
                -12, -8, -16, 3, 5, 8, 4, -5, -6, -8, -11, 4, 3, 13, 1, 2, 1, -1, 2, 7, 7, 7, 5, 4,
                -3, -5, -3, 11, 13, 13, 11, -3, 3, 8, 3, 13, 10, 18, 15, 12, 12, 8, 5,
            ],
            // Queen
            [
                -33, -28, -22, -43, -5, -32, -20, -41, -22, -23, -30, -16, -16, -23, -36, -32, -16,
                -27, 15, 6, 9, 17, 10, 5, -18, 28, 19, 47, 31, 34, 39, 23, 3, 22, 24, 45, 57, 40,
                57, 36, -20, 6, 9, 49, 47, 35, 19, 9, -17, 20, 32, 41, 58, 25, 30, 0, -9, 22, 22,
                27, 27, 19, 10, 20,
            ],
            // King
            [
                -53, -34, -21, -11, -28, -14, -24, -43, -27, -11, 4, 13, 14, 4, -5, -17, -19, -3,
                11, 21, 23, 16, 7, -9, -18, -4, 21, 24, 27, 23, 9, -11, -8, 22, 24, 27, 26, 33, 26,
                3, 10, 17, 23, 15, 20, 45, 44, 13, -12, 17, 14, 17, 17, 38, 23, 11, -74, -35, -18,
                -18, -11, 15, 4, -17,
            ],
        ];

        // Helper function to convert 2D board coordinates to 1D array index
        fn square_to_index(rank: usize, file: usize) -> usize {
            rank * 8 + file
        }

        // Piece type to index mapping
        fn piece_to_index(piece: Piece) -> usize {
            match piece {
                Piece::Pawn => 0,
                Piece::Knight => 1,
                Piece::Bishop => 2,
                Piece::Rook => 3,
                Piece::Queen => 4,
                Piece::King => 5,
            }
        }

        // Count pieces for game phase detection and other features
        let mut white_material_mg = 0;
        let mut black_material_mg = 0;
        let mut white_material_eg = 0;
        let mut black_material_eg = 0;
        let mut white_bishop_count = 0;
        let mut black_bishop_count = 0;
        let mut white_pawns_by_file = [0; 8];
        let mut black_pawns_by_file = [0; 8];
        let mut white_king_pos = (0, 0);
        let mut black_king_pos = (0, 0);

        // First pass: Count pieces and positions
        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.squares[rank][file] {
                    let piece_idx = piece_to_index(piece);

                    if color == Color::White {
                        if piece == Piece::Bishop {
                            white_bishop_count += 1;
                        } else if piece == Piece::King {
                            white_king_pos = (rank, file);
                        } else if piece == Piece::Pawn {
                            white_pawns_by_file[file] += 1;
                        }

                        white_material_mg += MATERIAL_MG[piece_idx];
                        white_material_eg += MATERIAL_EG[piece_idx];
                    } else {
                        if piece == Piece::Bishop {
                            black_bishop_count += 1;
                        } else if piece == Piece::King {
                            black_king_pos = (rank, file);
                        } else if piece == Piece::Pawn {
                            black_pawns_by_file[file] += 1;
                        }

                        black_material_mg += MATERIAL_MG[piece_idx];
                        black_material_eg += MATERIAL_EG[piece_idx];
                    }
                }
            }
        }

        // Calculate game phase based on remaining material
        let total_material_mg = white_material_mg + black_material_mg;
        let max_material = 2
            * (MATERIAL_MG[1] * 2
                + MATERIAL_MG[2] * 2
                + MATERIAL_MG[3] * 2
                + MATERIAL_MG[4]
                + MATERIAL_MG[0] * 8);
        let phase = (total_material_mg as f32) / (max_material as f32);

        // Clamp phase between 0 (endgame) and 1 (middlegame)
        let phase = phase.min(1.0).max(0.0);

        // Second pass: Evaluate pieces with position
        let mut mg_score = 0;
        let mut eg_score = 0;

        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.squares[rank][file] {
                    let piece_idx = piece_to_index(piece);

                    // Get 1D index for piece square tables
                    let sq_idx = if color == Color::White {
                        square_to_index(7 - rank, file) // White pieces are flipped vertically
                    } else {
                        square_to_index(rank, file) // Black pieces use the table as-is
                    };

                    let mg_value = MATERIAL_MG[piece_idx] + PST_MG[piece_idx][sq_idx];
                    let eg_value = MATERIAL_EG[piece_idx] + PST_EG[piece_idx][sq_idx];

                    if color == Color::White {
                        mg_score += mg_value;
                        eg_score += eg_value;
                    } else {
                        mg_score -= mg_value;
                        eg_score -= eg_value;
                    }
                }
            }
        }

        // Interpolate between middlegame and endgame scores based on phase
        let position_score = (phase * mg_score as f32 + (1.0 - phase) * eg_score as f32) as i32;
        score += position_score;

        // Additional evaluation factors

        // 1. Bishop pair bonus
        if white_bishop_count >= 2 {
            score += 30; // Bonus for having both bishops
        }
        if black_bishop_count >= 2 {
            score -= 30; // Same bonus for black
        }

        // 2. Rook on open files
        for file in 0..8 {
            for rank in 0..8 {
                if let Some((color, piece)) = self.squares[rank][file] {
                    if piece == Piece::Rook {
                        let file_pawns = white_pawns_by_file[file] + black_pawns_by_file[file];

                        if file_pawns == 0 {
                            // Open file
                            let bonus = 15;
                            if color == Color::White {
                                score += bonus;
                            } else {
                                score -= bonus;
                            }
                        } else if (color == Color::White && black_pawns_by_file[file] == 0)
                            || (color == Color::Black && white_pawns_by_file[file] == 0)
                        {
                            // Semi-open file
                            let bonus = 7;
                            if color == Color::White {
                                score += bonus;
                            } else {
                                score -= bonus;
                            }
                        }
                    }
                }
            }
        }

        // 3. Pawn structure
        for file in 0..8 {
            // Isolated pawns
            if white_pawns_by_file[file] > 0 {
                let left_file = if file > 0 {
                    white_pawns_by_file[file - 1]
                } else {
                    0
                };
                let right_file = if file < 7 {
                    white_pawns_by_file[file + 1]
                } else {
                    0
                };

                if left_file == 0 && right_file == 0 {
                    score -= 12; // Isolated pawn penalty
                }
            }

            if black_pawns_by_file[file] > 0 {
                let left_file = if file > 0 {
                    black_pawns_by_file[file - 1]
                } else {
                    0
                };
                let right_file = if file < 7 {
                    black_pawns_by_file[file + 1]
                } else {
                    0
                };

                if left_file == 0 && right_file == 0 {
                    score += 12; // Isolated pawn penalty for black
                }
            }

            // Doubled pawns penalty
            if white_pawns_by_file[file] > 1 {
                score -= 12 * (white_pawns_by_file[file] - 1); // Penalty for each doubled pawn
            }

            if black_pawns_by_file[file] > 1 {
                score += 12 * (black_pawns_by_file[file] - 1); // Penalty for black
            }

            // Passed pawns
            for rank in 0..8 {
                if let Some((Color::White, Piece::Pawn)) = self.squares[rank][file] {
                    let mut is_passed = true;

                    // Check if there are any black pawns ahead on same or adjacent files
                    for check_rank in 0..rank {
                        for check_file in file.saturating_sub(1)..=(file + 1).min(7) {
                            if let Some((Color::Black, Piece::Pawn)) =
                                self.squares[check_rank][check_file]
                            {
                                is_passed = false;
                                break;
                            }
                        }
                        if !is_passed {
                            break;
                        }
                    }

                    if is_passed {
                        // Bonus increases as pawn advances
                        let bonus = 10 + (7 - rank as i32) * 7;
                        score += bonus;
                    }
                } else if let Some((Color::Black, Piece::Pawn)) = self.squares[rank][file] {
                    let mut is_passed = true;

                    // Check if there are any white pawns ahead on same or adjacent files
                    for check_rank in (rank + 1)..8 {
                        for check_file in file.saturating_sub(1)..=(file + 1).min(7) {
                            if let Some((Color::White, Piece::Pawn)) =
                                self.squares[check_rank][check_file]
                            {
                                is_passed = false;
                                break;
                            }
                        }
                        if !is_passed {
                            break;
                        }
                    }

                    if is_passed {
                        // Bonus increases as pawn advances
                        let bonus = 10 + rank as i32 * 7;
                        score -= bonus;
                    }
                }
            }
        }

        // Return score relative to the current player to move
        if self.white_to_move {
            score
        } else {
            -score
        }
    }

    // --- Search Functions (Refactored) ---

    fn negamax(
        &mut self,
        tt: &mut TranspositionTable, // Pass TT
        depth: u32,
        mut alpha: i32, // Keep mutable
        mut beta: i32,  // Keep mutable
    ) -> i32 {
        let original_alpha = alpha; // Store original alpha for TT bounds
        let current_hash = self.hash; // Get hash for current position

        // --- Transposition Table Probe ---
        let mut hash_move: Option<Move> = None;
        if let Some(entry) = tt.probe(current_hash) {
            if entry.depth >= depth {
                // Use entry only if depth is sufficient
                match entry.bound_type {
                    BoundType::Exact => return entry.score, // Found exact score
                    BoundType::LowerBound => alpha = alpha.max(entry.score), // Update alpha
                    BoundType::UpperBound => beta = beta.min(entry.score), // Update beta
                }
                if alpha >= beta {
                    return entry.score; // Cutoff based on TT entry
                }
            }
            // Use best move from TT for move ordering, even if depth wasn't sufficient
            // Must clone the move here as entry is borrowed
            hash_move = entry.best_move.clone();
        }

        // --- Base Case: Depth 0 ---
        if depth == 0 {
            // Call quiescence search at leaf nodes
            return self.quiesce(tt, alpha, beta); // Pass TT to quiesce
        }

        // --- Generate Moves ---
        let mut legal_moves = self.generate_moves(); // Needs &mut self
        legal_moves.sort_by_key(|m| -mvv_lva_score(m, self));

        // --- Check for Checkmate / Stalemate ---
        if legal_moves.is_empty() {
            let current_color = self.current_color();
            return if self.is_in_check(current_color) {
                -(MATE_SCORE - (100 - depth as i32)) // Checkmate score depends on depth
            } else {
                0 // Stalemate
            };
        }

        // --- Move Ordering ---
        // Basic: Try hash move first if available
        if let Some(hm) = &hash_move {
            if let Some(pos) = legal_moves.iter().position(|m| m == hm) {
                // Swap hash move to the front
                legal_moves.swap(0, pos);
            }
        }
        // TODO: Implement more sophisticated move ordering (captures, killers, history)

        // --- Iterate Through Moves ---
        let mut best_score = -MATE_SCORE * 2; // Initialize with very low score
        let mut best_move_found: Option<Move> = None;

        for (i, m) in legal_moves.iter().enumerate() {
            let info = self.make_move(&m);
            let score = if i == 0 {
                -self.negamax(tt, depth - 1, -beta, -alpha)
            } else {
                let mut score = -self.negamax(tt, depth - 1, -alpha - 1, -alpha);
                if score > alpha && score < beta {
                    score = -self.negamax(tt, depth - 1, -beta, -alpha);
                }
                score
            };
            self.unmake_move(&m, info);

            // --- Update Alpha/Beta and Best Score ---
            if score > best_score {
                best_score = score;
                best_move_found = Some(m.clone()); // Store the best move *found* so far
            }

            alpha = alpha.max(best_score);

            if alpha >= beta {
                // Beta Cutoff
                // TODO: Store killer moves here if implementing that heuristic
                break;
            }
        }

        // --- Transposition Table Store ---
        let bound_type = if best_score <= original_alpha {
            BoundType::UpperBound // Failed low (score <= alpha), so it's an upper bound for future searches
        } else if best_score >= beta {
            BoundType::LowerBound // Failed high (score >= beta), so it's a lower bound
        } else {
            BoundType::Exact // Score is within the alpha-beta window
        };

        tt.store(current_hash, depth, best_score, bound_type, best_move_found); // Store result

        best_score
    }

    // Quiescence search (also takes TT, but primarily for passing down)
    fn quiesce(
        &mut self,
        tt: &mut TranspositionTable, // Pass TT along (though not used directly here yet)
        mut alpha: i32,
        beta: i32,
    ) -> i32 {
        // --- Standing Pat Score ---
        let stand_pat_score = self.evaluate();

        // --- Alpha-Beta Pruning Check (Standing Pat) ---
        if stand_pat_score >= beta {
            return beta; // Fail-high
        }
        alpha = alpha.max(stand_pat_score); // Update lower bound

        // --- Generate Only Tactical Moves ---
        let mut tactical_moves = self.generate_tactical_moves();
        tactical_moves.sort_by_key(|m| -mvv_lva_score(m, self));

        // TODO: Add move ordering for tactical moves (e.g., MVV-LVA)

        // --- Iterate Through Tactical Moves ---
        let mut best_score = stand_pat_score; // Start with the standing pat score

        for m in tactical_moves {
            let info = self.make_move(&m);
            // Recursive call passes TT, alpha, beta
            let score = -self.quiesce(tt, -beta, -alpha);
            self.unmake_move(&m, info);

            best_score = best_score.max(score);
            alpha = alpha.max(best_score);

            if alpha >= beta {
                break; // Beta cutoff
            }
        }

        // Note: We typically don't store quiescence results directly in the main TT
        // in the same way as fixed-depth search, as the 'depth' concept is different.
        // The result is implicitly stored when negamax calls quiesce at depth 0.

        alpha // Return the best score found (alpha)
    }

    // Add a function to generate only tactical moves (captures, promotions)
    // This function needs &mut self because legality checking involves make/unmake
    fn generate_tactical_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color();

        // 1. Generate Pseudo-Tactical Moves (faster than generating all pseudo moves)
        let mut pseudo_tactical_moves = Vec::new();
        for r in 0..8 {
            for f in 0..8 {
                if let Some((c, piece)) = self.squares[r][f] {
                    if c == current_color {
                        let from = Square(r, f);
                        // Generate moves for this piece, but only keep captures/promotions
                        match piece {
                            Piece::Pawn => {
                                // Check pawn captures and promotions specifically
                                self.generate_pawn_tactical_moves(from, &mut pseudo_tactical_moves);
                            }
                            _ => {
                                // For other pieces, generate their moves and filter
                                let piece_moves = self.generate_piece_moves(from, piece);
                                for m in piece_moves {
                                    // Keep captures (or en passant)
                                    if m.captured_piece.is_some() || m.is_en_passant {
                                        pseudo_tactical_moves.push(m);
                                    }
                                    // Note: Promotions are handled by generate_pawn_tactical_moves
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Check Legality (Filter out moves that leave the king in check)
        let mut legal_tactical_moves = Vec::new();
        for m in pseudo_tactical_moves {
            // Skip castling as it's not typically considered a tactical move for quiescence
            if m.is_castling {
                continue;
            }

            // Check general legality: Does the move leave the king in check?
            let info = self.make_move(&m); // Make the move temporarily
            if !self.is_in_check(current_color) {
                // Check if the player who moved is now safe
                legal_tactical_moves.push(m.clone()); // If safe, it's a legal tactical move
            }
            self.unmake_move(&m, info); // Unmake the move to restore state
        }

        legal_tactical_moves
    }

    // Helper specifically for pawn captures and promotions
    fn generate_pawn_tactical_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.current_color();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        let r = from.0 as isize;
        let f = from.1 as isize;

        let forward_r = r + dir;

        // Check Promotions (forward move resulting in promotion)
        if forward_r >= 0 && forward_r < 8 {
            let forward_sq = Square(forward_r as usize, f as usize);
            if forward_sq.0 == promotion_rank && self.squares[forward_sq.0][forward_sq.1].is_none()
            {
                for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                    moves.push(self.create_move(from, forward_sq, Some(promo), false, false));
                }
            }
        }

        // Check Captures and Capture-Promotions
        if forward_r >= 0 && forward_r < 8 {
            for df in [-1, 1] {
                let capture_f = f + df;
                if capture_f >= 0 && capture_f < 8 {
                    let target_sq = Square(forward_r as usize, capture_f as usize);

                    // Regular capture
                    if let Some((target_color, _)) = self.squares[target_sq.0][target_sq.1] {
                        if target_color != color {
                            if target_sq.0 == promotion_rank {
                                // Capture with promotion
                                for promo in
                                    [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
                                {
                                    moves.push(self.create_move(
                                        from,
                                        target_sq,
                                        Some(promo),
                                        false,
                                        false,
                                    ));
                                }
                            } else {
                                // Normal capture
                                moves.push(self.create_move(from, target_sq, None, false, false));
                            }
                        }
                    }
                    // En passant capture
                    else if Some(target_sq) == self.en_passant_target {
                        // Check if the en passant target square matches the potential capture square
                        moves.push(self.create_move(from, target_sq, None, false, true));
                    }
                }
            }
        }
    }


    // --- Utility Functions ---
    fn current_color(&self) -> Color {
        if self.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    fn opponent_color(&self, color: Color) -> Color {
        match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    // Add a print function for debugging
    fn print(&self) {
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let piece_char = match self.squares[rank][file] {
                    Some((Color::White, Piece::Pawn)) => 'P',
                    Some((Color::White, Piece::Knight)) => 'N',
                    Some((Color::White, Piece::Bishop)) => 'B',
                    Some((Color::White, Piece::Rook)) => 'R',
                    Some((Color::White, Piece::Queen)) => 'Q',
                    Some((Color::White, Piece::King)) => 'K',
                    Some((Color::Black, Piece::Pawn)) => 'p',
                    Some((Color::Black, Piece::Knight)) => 'n',
                    Some((Color::Black, Piece::Bishop)) => 'b',
                    Some((Color::Black, Piece::Rook)) => 'r',
                    Some((Color::Black, Piece::Queen)) => 'q',
                    Some((Color::Black, Piece::King)) => 'k',
                    None => ' ',
                };
                print!(" {} |", piece_char);
            }
            println!("\n  +---+---+---+---+---+---+---+---+");
        }
        println!("    a   b   c   d   e   f   g   h");
        println!(
            "Turn: {}",
            if self.white_to_move { "White" } else { "Black" }
        );
        if let Some(ep_target) = self.en_passant_target {
            println!("EP Target: {}", format_square(ep_target));
        }
        println!("Castling: {:?}", self.castling_rights); // Consider formatting this better
        println!("------------------------------------");
    }
} // end impl Board

// Parses a move in UCI format (e.g., "e2e4", "e7e8q")
// Needs the current board state to find the matching legal move object.
fn parse_uci_move(board: &mut Board, uci_string: &str) -> Option<Move> {
    if uci_string.len() < 4 || uci_string.len() > 5 {
        return None; // Invalid length
    }

    let from_chars: Vec<char> = uci_string.chars().take(2).collect();
    let to_chars: Vec<char> = uci_string.chars().skip(2).take(2).collect();

    if from_chars.len() != 2 || to_chars.len() != 2 {
        return None; // Should not happen with length check, but be safe
    }

    // Basic validation of chars
    if !('a'..='h').contains(&from_chars[0])
        || !('1'..='8').contains(&from_chars[1])
        || !('a'..='h').contains(&to_chars[0])
        || !('1'..='8').contains(&to_chars[1])
    {
        return None; // Invalid algebraic notation characters
    }

    let from_file = file_to_index(from_chars[0]);
    let from_rank = rank_to_index(from_chars[1]);
    let to_file = file_to_index(to_chars[0]);
    let to_rank = rank_to_index(to_chars[1]);

    let from_sq = Square(from_rank, from_file);
    let to_sq = Square(to_rank, to_file);

    // Handle promotion
    let promotion_piece = if uci_string.len() == 5 {
        match uci_string.chars().nth(4) {
            Some('q') => Some(Piece::Queen),
            Some('r') => Some(Piece::Rook),
            Some('b') => Some(Piece::Bishop),
            Some('n') => Some(Piece::Knight),
            _ => return None, // Invalid promotion character
        }
    } else {
        None
    };

    // Find the matching legal move
    // We need generate_moves, which takes &mut self. This is slightly awkward
    // if we just want to *find* the move without changing state yet.
    // A temporary clone *might* be acceptable here, or we pass the pre-generated list.
    // Let's generate moves here.
    let legal_moves = board.generate_moves(); // Needs &mut borrow

    for legal_move in legal_moves {
        if legal_move.from == from_sq && legal_move.to == to_sq {
            // Check for promotion match
            if legal_move.promotion == promotion_piece {
                // Found the move! Return a clone of it.
                return Some(legal_move.clone());
            }
            // If no promotion specified by user AND move is not a promotion, it's a match
            else if promotion_piece.is_none() && legal_move.promotion.is_none() {
                return Some(legal_move.clone());
            }
        }
    }

    None // No matching legal move found
}

fn find_best_move(board: &mut Board, tt: &mut TranspositionTable, max_depth: u32) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut best_score = -MATE_SCORE * 2;

    let legal_moves = board.generate_moves();
    if legal_moves.is_empty() {
        return None;
    }
    if legal_moves.len() == 1 {
        return Some(legal_moves[0]); // No need to search further
    }
    let mut root_moves = legal_moves.clone(); // Reuse for move ordering

    // Iterative Deepening Loop
    for depth in 1..=max_depth {
        let mut alpha = -MATE_SCORE * 2;
        let beta = MATE_SCORE * 2;
        let mut current_best_score = -MATE_SCORE * 2;
        let mut current_best_move: Option<Move> = None;

        // Optional: order moves using hash move from TT
        if let Some(entry) = tt.probe(board.hash) {
            if let Some(hm) = &entry.best_move {
                if let Some(pos) = root_moves.iter().position(|m| m == hm) {
                    root_moves.swap(0, pos);
                }
            }
        }

        for m in &root_moves {
            let info = board.make_move(m);
            let score = -board.negamax(tt, depth - 1, -beta, -alpha);
            board.unmake_move(m, info);

            if score > current_best_score {
                current_best_score = score;
                current_best_move = Some(*m);
            }

            alpha = alpha.max(current_best_score);
        }

        if let Some(mv) = current_best_move {
            best_score = current_best_score;
            best_move = Some(mv);

            // Optional: reorder root_moves so best move is searched first in next iteration
            if let Some(pos) = root_moves.iter().position(|m| *m == mv) {
                root_moves.swap(0, pos);
            }
        }
    }

    best_move
}

fn format_square(sq: Square) -> String {
    format!("{}{}", (sq.1 as u8 + b'a') as char, sq.0 + 1)
}

fn parse_position_command(board: &mut Board, parts: &[&str]) {
    let mut i = 1;
    if i < parts.len() && parts[i] == "startpos" {
        *board = Board::new();
        i += 1;
    } else if i < parts.len() && parts[i] == "fen" {
        let fen = parts[i + 1..i + 7].join(" ");
        *board = Board::from_fen(&fen);
        i += 7;
    }

    if i < parts.len() && parts[i] == "moves" {
        i += 1;
        while i < parts.len() {
            if let Some(mv) = parse_uci_move(board, parts[i]) {
                board.make_move(&mv);
            } else {
                eprintln!("Invalid move: {}", parts[i]);
            }
            i += 1;
        }
    }
}

fn format_uci_move(mv: &Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => '?',
        });
    }
    s
}

fn find_best_move_with_time(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_time: Duration,
    start_time: Instant,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut depth = 1;
    let mut last_depth_time = Duration::from_millis(1); // Prevent div-by-zero on first estimate

    const SAFETY_MARGIN: Duration = Duration::from_millis(5);
    const TIME_GROWTH_FACTOR: f32 = 2.0; // Each depth takes ~2× longer

    while start_time.elapsed() + SAFETY_MARGIN < max_time {
        let elapsed = start_time.elapsed();
        let time_remaining = max_time.checked_sub(elapsed).unwrap_or_default();

        // Estimate whether we have enough time for the next depth
        let estimated_next_time = last_depth_time.mul_f32(TIME_GROWTH_FACTOR);
        if estimated_next_time + SAFETY_MARGIN > time_remaining {
            break; // Not enough time for another full depth
        }

        let depth_start = Instant::now();

        let mut alpha = -MATE_SCORE * 2;
        let beta = MATE_SCORE * 2;
        let mut best_score = -MATE_SCORE * 2;
        let mut legal_moves = board.generate_moves();

        if legal_moves.is_empty() {
            return None;
        }

        if legal_moves.len() == 1 {
            return Some(legal_moves[0]); // No need to search further
        }

        // MVV-LVA and TT move ordering
        legal_moves.sort_by_key(|m| -mvv_lva_score(m, board));
        if let Some(entry) = tt.probe(board.hash) {
            if let Some(hm) = &entry.best_move {
                if let Some(pos) = legal_moves.iter().position(|m| m == hm) {
                    legal_moves.swap(0, pos);
                }
            }
        }

        let mut new_best_move = None;

        for m in &legal_moves {
            if start_time.elapsed() + SAFETY_MARGIN >= max_time {
                break;
            }

            let info = board.make_move(m);
            let score = -board.negamax(tt, depth - 1, -beta, -alpha);
            board.unmake_move(m, info);

            if score > best_score {
                best_score = score;
                new_best_move = Some(*m);
            }

            alpha = alpha.max(best_score);
        }

        // Only update result if completed full depth in time
        if start_time.elapsed() + SAFETY_MARGIN < max_time {
            best_move = new_best_move;
            last_depth_time = depth_start.elapsed();
            depth += 1;
        } else {
            break;
        }
    }

    best_move
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut board = Board::new();
    let mut tt = TranspositionTable::new(1024); // 1024MB TT
    let search_depth = 6;

    let mut time_left = Duration::from_secs(5); // fallback
    let mut inc = Duration::ZERO;
    let mut movetime: Option<Duration> = None;

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "uci" => {
                println!("id name MyRustEngine");
                println!("id author Dean Menezes");
                println!("uciok");
            }
            "isready" => {
                println!("readyok");
            }
            "ucinewgame" => {
                board = Board::new();
            }
            "position" => {
                parse_position_command(&mut board, &parts);
            }
            "go" => {
                let mut i = 1;
                while i < parts.len() {
                    match parts[i] {
                        "wtime" if board.white_to_move => {
                            time_left = Duration::from_millis(parts[i + 1].parse().unwrap_or(1000));
                            i += 2;
                        }
                        "btime" if !board.white_to_move => {
                            time_left = Duration::from_millis(parts[i + 1].parse().unwrap_or(1000));
                            i += 2;
                        }
                        "winc" if board.white_to_move => {
                            inc = Duration::from_millis(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "binc" if !board.white_to_move => {
                            inc = Duration::from_millis(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "movetime" => {
                            movetime =
                                Some(Duration::from_millis(parts[i + 1].parse().unwrap_or(100)));
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }

                let max_time = movetime.unwrap_or_else(|| time_left / 30 + inc);
                let start = Instant::now();

                if let Some(best_move) =
                    find_best_move_with_time(&mut board, &mut tt, max_time, start)
                {
                    let uci_move = format_uci_move(&best_move);
                    println!("bestmove {}", uci_move);
                } else {
                    println!("bestmove 0000");
                }
            }
            "quit" => break,
            _ => {
                // Ignore unknown commands or log them if needed
            }
        }

        stdout.flush().unwrap();
    }
}

#[cfg(test)]
mod perft_tests {
    use super::*;

    struct TestPosition {
        name: &'static str,
        fen: &'static str,
        depths: &'static [(usize, u64)], // (depth, expected node count)
    }

    // Common test positions with known perft results
    const TEST_POSITIONS: &[TestPosition] = &[
        // Initial position
        TestPosition {
            name: "Initial Position",
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            depths: &[
                (1, 20),      // 20 possible moves from initial position
                (2, 400),     // 400 positions after 2 plies
                (3, 8902),    // 8,902 positions after 3 plies
                (4, 197281),  // 197,281 positions after 4 plies
                (5, 4865609), // 4,865,609 positions after 5 plies
            ],
        },
        // Position 2 (Kiwipete)
        TestPosition {
            name: "Kiwipete",
            fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            depths: &[(1, 48), (2, 2039), (3, 97862), (4, 4085603)],
        },
        // Position 3
        TestPosition {
            name: "Position 3",
            fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            depths: &[(1, 14), (2, 191), (3, 2812), (4, 43238), (5, 674624)],
        },
        // Position 4
        TestPosition {
            name: "Position 4",
            fen: "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            depths: &[(1, 6), (2, 264), (3, 9467), (4, 422333)],
        },
        // Position 5
        TestPosition {
            name: "Position 5",
            fen: "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            depths: &[(1, 44), (2, 1486), (3, 62379), (4, 2103487)],
        },
        // Position 6 (Win at Chess)
        TestPosition {
            name: "Position 6 (Win at Chess)",
            fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            depths: &[
                (1, 46),
                (2, 2079),
                (3, 89890),
                //(4, 3894594), // Commented out as it may take too long
            ],
        },
        // Additional edge cases
        TestPosition {
            name: "En Passant Capture",
            fen: "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
            depths: &[
                (1, 31), // Includes en passant capture
                (2, 707),
                (3, 21637),
            ],
        },
        TestPosition {
            name: "Promotion",
            fen: "n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1",
            depths: &[
                (1, 24), // Many promotion options
                (2, 496),
                (3, 9483),
            ],
        },
        TestPosition {
            name: "Castling",
            fen: "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
            depths: &[
                (1, 26), // Both sides can castle in both directions
                (2, 568),
                (3, 13744),
            ],
        },
    ];

    #[test]
    fn test_all_perft_positions() {
        for position in TEST_POSITIONS {
            println!("Testing position: {}", position.name);
            println!("FEN: {}", position.fen);

            let mut board = Board::from_fen(position.fen);

            for &(depth, expected) in position.depths {
                let start = Instant::now();
                let nodes = board.perft(depth);
                let duration = start.elapsed();

                println!("  Depth {}: {} nodes in {:?}", depth, nodes, duration);

                assert_eq!(
                    nodes, expected,
                    "Perft failed for position '{}' at depth {}. Expected: {}, Got: {}",
                    position.name, depth, expected, nodes
                );
            }
            println!("------------------------------");
        }
    }
}
