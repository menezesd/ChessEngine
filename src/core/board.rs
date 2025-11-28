use crate::magic;
use crate::core::types::{
    bitboard_for_square, format_square, Bitboard,
    Color, Piece, Square,
};
use crate::core::zobrist::{
    color_to_zobrist_index, piece_to_zobrist_index, ZOBRIST,
};
use crate::core::bitboard::{BitboardUtils, castling_bit, color_from_index, piece_from_index};
use crate::core::bitboard::{CASTLE_WHITE_KINGSIDE, CASTLE_WHITE_QUEENSIDE, CASTLE_BLACK_KINGSIDE, CASTLE_BLACK_QUEENSIDE};


// --- Constants for array indices and magic numbers ---
/// White's starting rank
const WHITE_START_RANK: usize = 0;
/// Black's starting rank
const BLACK_START_RANK: usize = 7;
/// White's pawn starting rank
const WHITE_PAWN_RANK: usize = 1;
/// Black's pawn starting rank
const BLACK_PAWN_RANK: usize = 6;

/// Type-safe piece index for array access
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PieceIndex {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceIndex {
    pub const fn as_usize(self) -> usize {
        self as usize
    }

    pub const fn count() -> usize {
        6 // Pawn, Knight, Bishop, Rook, Queen, King
    }
}

/// Type-safe color index for array access
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum ColorIndex {
    White = 0,
    Black = 1,
}

impl ColorIndex {
    pub const fn as_usize(self) -> usize {
        self as usize
    }

    pub const fn count() -> usize {
        2 // White, Black
    }
}

/// Information required to restore a position after `make_move`.
///
/// This struct is returned by `Board::make_move` and passed to `Board::unmake_move`.
/// It stores only the minimal snapshot needed to restore invariants (hash, clocks,
/// captured piece, and history length).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnmakeInfo {
    /// Captured piece (color, piece) if the move captured something.
    pub captured_piece_info: Option<(Color, Piece)>,
    /// Previous en-passant target square (if any).
    pub previous_en_passant_target: Option<Square>,
    /// Previous castling rights bitmask.
    pub previous_castling_rights: u8,
    /// Previous full position hash (Zobrist) — restored directly for correctness.
    pub previous_hash: u64,
    /// Previous halfmove clock value.
    pub previous_halfmove_clock: u32,
    /// Length of the position history prior to the move (used to truncate back).
    pub previous_history_len: usize,
    /// Previous move made (for recapture extension, etc.)
    pub previous_last_move: Option<crate::core::types::Move>,
}

/// Minimal snapshot for a null-move so the position can be restored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NullUnmake {
    pub previous_en_passant_target: Option<Square>,
    pub previous_hash: u64,
    pub previous_halfmove_clock: u32,
    pub previous_history_len: usize,
}

// piece values moved to `ordering.rs` to centralize move ordering heuristics.

#[derive(Clone, Debug)]
pub struct Board {
    pub bitboards: [[Bitboard; PieceIndex::count()]; ColorIndex::count()],
    pub occupancy: [Bitboard; ColorIndex::count()],
    pub all_occupancy: Bitboard,
    pub white_to_move: bool,
    pub en_passant_target: Option<Square>,
    pub castling_rights: u8,
    pub hash: u64,
    pub halfmove_clock: u32,
    pub position_history: Vec<u64>,
    pub last_move_made: Option<crate::core::types::Move>,
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    // === STATIC UTILITIES ===
    // Utility functions that don't require a board instance

    /// Convert a 0-based square index (0..63) to a `Square` (rank, file).
    ///
    /// This helper is handy when iterating bitboards and converting
    /// trailing-zero indices into board coordinates.
    pub const fn square_from_index(index: usize) -> Square {
        BitboardUtils::square_from_index(index)
    }

    pub const fn file_mask(file: usize) -> Bitboard {
        BitboardUtils::file_mask(file)
    }

    // `file_mask_allow` was unused; removed to trim dead code. Use `Board::file_mask` instead.

    pub fn knight_attacks(square: Square) -> Bitboard {
        BitboardUtils::knight_attacks(square)
    }
    pub fn king_attacks(square: Square) -> Bitboard {
        BitboardUtils::king_attacks(square)
    }
    pub fn rook_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
        magic::rook_attacks(square, occupancy)
    }
    pub fn bishop_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
        magic::bishop_attacks(square, occupancy)
    }

    // === BOARD CONSTRUCTION ===

    pub fn empty() -> Self {
        Board {
            bitboards: [[0; PieceIndex::count()]; ColorIndex::count()],
            occupancy: [0; ColorIndex::count()],
            all_occupancy: 0,
            white_to_move: true,
            en_passant_target: None,
            castling_rights: 0,
            hash: 0,
            halfmove_clock: 0,
            position_history: Vec::new(),
            last_move_made: None,
        }
    }

    // Helper to iterate set bit indices in a bitboard (LSB-first)
    fn bits_iter(mut bb: Bitboard) -> impl Iterator<Item = usize> {
        std::iter::from_fn(move || {
            if bb == 0 {
                None
            } else {
                let idx = bb.trailing_zeros() as usize;
                bb &= bb - 1;
                Some(idx)
            }
        })
    }

    // all_occupancy is updated incrementally in place_piece_at and remove_piece_at



    fn remove_piece_at(&mut self, square: Square) -> Option<(Color, Piece)> {
        let mask = bitboard_for_square(square);
        for color_idx in 0..ColorIndex::count() {
            if self.occupancy[color_idx] & mask != 0 {
                for piece_idx in 0..PieceIndex::count() {
                    if self.bitboards[color_idx][piece_idx] & mask != 0 {
                        self.bitboards[color_idx][piece_idx] &= !mask;
                        self.occupancy[color_idx] &= !mask;
                        // incremental update to combined occupancy
                        self.all_occupancy &= !mask;
                        return Some((color_from_index(color_idx), piece_from_index(piece_idx)));
                    }
                }
            }
        }
        None
    }

    pub fn place_piece_at(&mut self, square: Square, piece: (Color, Piece)) {
        let mask = bitboard_for_square(square);
        let color_idx = color_to_zobrist_index(piece.0);
        let piece_idx = piece_to_zobrist_index(piece.1);
        self.bitboards[color_idx][piece_idx] |= mask;
        self.occupancy[color_idx] |= mask;
        // incremental update to combined occupancy
        self.all_occupancy |= mask;
    }

    fn set_piece_at(
        &mut self,
        square: Square,
        piece: Option<(Color, Piece)>,
    ) -> Option<(Color, Piece)> {
        let previous = self.remove_piece_at(square);
        if let Some(info) = piece {
            self.place_piece_at(square, info);
        }
        previous
    }

    pub(crate) fn set_square(
        &mut self,
        rank: usize,
        file: usize,
        piece: Option<(Color, Piece)>,
    ) -> Option<(Color, Piece)> {
        self.set_piece_at(Square(rank, file), piece)
    }

    pub(crate) fn add_castling_right(&mut self, color: Color, side: char) {
        self.castling_rights |= castling_bit(color, side);
    }

    pub fn new() -> Self {
        let mut board = Board::empty();
        board.castling_rights = CASTLE_WHITE_KINGSIDE
            | CASTLE_WHITE_QUEENSIDE
            | CASTLE_BLACK_KINGSIDE
            | CASTLE_BLACK_QUEENSIDE;

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
            board.place_piece_at(Square(WHITE_START_RANK, i), (Color::White, *piece));
            board.place_piece_at(Square(BLACK_START_RANK, i), (Color::Black, *piece));
            board.place_piece_at(Square(WHITE_PAWN_RANK, i), (Color::White, Piece::Pawn));
            board.place_piece_at(Square(BLACK_PAWN_RANK, i), (Color::Black, Piece::Pawn));
        }
        board.hash = board.calculate_initial_hash(); // Calculate hash after setting up board
        board.halfmove_clock = 0;
        board.position_history.clear();
        board.position_history.push(board.hash);
        board.last_move_made = None;
        board
    }

    pub fn from_fen(fen: &str) -> Self {
        // Delegate to fallible parser and preserve previous panic behavior for callers
        match Board::try_from_fen(fen) {
            Ok(b) => b,
            Err(e) => panic!("from_fen failed: {:?}", e),
        }
    }

    pub fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;

        for color_idx in 0..ColorIndex::count() {
            for piece_idx in 0..PieceIndex::count() {
                let bb = self.bitboards[color_idx][piece_idx];
                for sq_idx in Self::bits_iter(bb) {
                    hash ^= ZOBRIST.piece_keys[piece_idx][color_idx][sq_idx];
                }
            }
        }

        // Side to move
        if !self.white_to_move {
            hash ^= ZOBRIST.black_to_move_key;
        }

        // Castling rights
        const KINGSIDE: usize = 0;
        const QUEENSIDE: usize = 1;
        
        for color_idx in 0..ColorIndex::count() {
            let color = color_from_index(color_idx);
            if self.has_castling_right(color, 'K') {
                hash ^= ZOBRIST.castling_keys[color_idx][KINGSIDE];
            }
            if self.has_castling_right(color, 'Q') {
                hash ^= ZOBRIST.castling_keys[color_idx][QUEENSIDE];
            }
        }

        // En passant target
        if let Some(ep_square) = self.en_passant_target {
            hash ^= ZOBRIST.en_passant_keys[ep_square.1]; // XOR based on the file
        }

        hash
    }

    // === MOVE LOGIC ===
    // Methods for making and unmaking moves

    // --- Move Generation (largely unchanged logic, but uses new Move struct) ---
    // Provide "into" variants that accept a reusable buffer to avoid allocations.

    // Removed unused `generate_pseudo_moves` (use `generate_pseudo_moves_into` to avoid allocation).

    // --- Move Generation (largely unchanged logic, but uses new Move struct) ---
    // Provide "into" variants that accept a reusable buffer to avoid allocations.

    // Removed unused `generate_pseudo_moves` (use `generate_pseudo_moves_into` to avoid allocation).

    #[allow(clippy::too_many_arguments)]
    // Generates only fully legal moves, takes &mut self
    pub fn generate_moves_into(&mut self, out: &mut crate::core::types::MoveList) {
        let mut pseudo_moves = crate::core::types::MoveList::new();
        crate::movegen::MoveGen::generate_pseudo_moves_into(self, &mut pseudo_moves);
        self.filter_illegal_moves_into(&mut pseudo_moves, out);
    }

    /// Helper to filter out pseudo-legal moves that leave the king in check.
    fn filter_illegal_moves_into(&mut self, pseudo_moves: &mut crate::core::types::MoveList, out: &mut crate::core::types::MoveList) {
        let current_color = self.current_color();
        for m in pseudo_moves.drain(..) { // Use drain to efficiently move elements
            let info = self.make_move(&m);
            let is_legal = !self.is_in_check(current_color);
            self.unmake_move(&m, info);
            if is_legal {
                out.push(m);
            }
        }
    }

    // === TACTICAL MOVE GENERATION ===
    // Methods for generating tactical moves (captures, promotions, etc.)

    // Allocation-returning tactical move generator removed; use `generate_tactical_moves_into`.

    pub fn generate_tactical_moves_into(&mut self, out: &mut crate::core::types::MoveList) {
        let mut pseudo_tactical = crate::core::types::MoveList::new();
        crate::movegen::MoveGen::generate_tactical_moves_into(self, &mut pseudo_tactical);
        self.filter_illegal_moves_into(&mut pseudo_tactical, out);
    }

    // === PERFORMANCE TESTING ===
    // Methods for testing move generation performance

    // --- Perft (for testing, now takes &mut self) ---
    pub fn perft(&mut self, depth: usize) -> u64 {
        crate::perft::Perft::perft(self, depth)
    }

    // === UTILITY FUNCTIONS ===
    // Miscellaneous utility methods

    /// Returns the color whose turn it currently is.
    ///
    /// Convenience accessor: `Color::White` when `white_to_move` is true,
    /// otherwise `Color::Black`.
    pub const fn current_color(&self) -> Color {
        if self.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    pub const fn opponent_color(color: Color) -> Color {
        match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    /// Checks if the current position is a candidate for endgame tablebase lookup.
    /// Returns true if the total number of pieces (excluding kings) is less than or equal to 3.
    pub fn is_tablebase_endgame(&self) -> bool {
        let mut total_pieces = 0;
        for color_idx in 0..ColorIndex::count() {
            for piece_idx in 0..PieceIndex::count() {
                if piece_idx != piece_to_zobrist_index(Piece::King) {
                    total_pieces += self.bitboards[color_idx][piece_idx].count_ones();
                }
            }
        }
        total_pieces <= 3
    }

    // === DEBUGGING AND DISPLAY ===
    // Methods for debugging and displaying the board state

    /// Returns the FEN-style castling rights string for the current board state.
    fn get_castling_string(&self) -> String {
        let mut castling_str = String::new();
        if self.has_castling_right(Color::White, 'K') {
            castling_str.push('K');
        }
        if self.has_castling_right(Color::White, 'Q') {
            castling_str.push('Q');
        }
        if self.has_castling_right(Color::Black, 'K') {
            castling_str.push('k');
        }
        if self.has_castling_right(Color::Black, 'Q') {
            castling_str.push('q');
        }
        if castling_str.is_empty() {
            "-".to_string()
        } else {
            castling_str
        }
    }

    pub fn to_fen(&self) -> String {
        let mut fen = String::new();

        // Piece placement
        for rank in (0..8).rev() {
            let mut empty_squares = 0;
            for file in 0..8 {
                match self.get_square(rank, file) {
                    Some((color, piece)) => {
                        if empty_squares > 0 {
                            fen.push_str(&empty_squares.to_string());
                            empty_squares = 0;
                        }
                        fen.push(Board::piece_to_char(color, piece));
                    }
                    None => {
                        empty_squares += 1;
                    }
                }
            }
            if empty_squares > 0 {
                fen.push_str(&empty_squares.to_string());
            }
            if rank > 0 {
                fen.push('/');
            }
        }

        // Side to move
        fen.push(' ');
        fen.push(if self.white_to_move { 'w' } else { 'b' });

        // Castling rights
        fen.push(' ');
        fen.push_str(&self.get_castling_string());

        // En passant target square
        fen.push(' ');
        if let Some(ep_target) = self.en_passant_target {
            fen.push_str(&format_square(ep_target));
        } else {
            fen.push('-');
        }

        // Halfmove clock
        fen.push(' ');
        fen.push_str(&self.halfmove_clock.to_string());

        // Fullmove number (always 1 for now, as we don't track it)
        fen.push(' ');
        fen.push_str("1"); // Assuming start of game for simplicity in FEN conversion, can be updated if fullmove tracked

        fen
    }

    // Add a print function for debugging and make it public so callers/tests can use it.
    pub fn print(&self) {
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let piece_char = match self.get_square(rank, file) {
                    Some((color, piece)) => Board::piece_to_char(color, piece),
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
        println!("Castling: {}", self.get_castling_string());
        println!("------------------------------------");
    }
} // end impl Board

// Parses a move in UCI format (e.g., "e2e4", "e7e8q")

// Tests moved to `tests/board_tests.rs` to separate production and test code

#[cfg(test)]
mod tests {
    use super::Board;

    #[test]
    fn board_print_is_used_in_tests() {
        // Call the public print helper so it's considered used by the library
        let b = Board::new();
        b.print();
    }
}
