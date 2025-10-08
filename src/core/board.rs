use crate::magic;
use crate::transposition::transposition_table::TranspositionTable;
use crate::core::types::{
    bitboard_for_square, file_to_index, format_square, rank_to_index, Bitboard,
    Color, Move, Piece, Square,
};
use crate::core::zobrist::{
    color_to_zobrist_index, piece_to_zobrist_index, square_to_zobrist_index, ZOBRIST,
};
use crate::core::bitboard::{BitboardUtils, castling_bit, color_from_index, piece_from_index};
use crate::core::bitboard::{CASTLE_WHITE_KINGSIDE, CASTLE_WHITE_QUEENSIDE, CASTLE_BLACK_KINGSIDE, CASTLE_BLACK_QUEENSIDE};

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
    pub bitboards: [[Bitboard; 6]; 2],
    pub occupancy: [Bitboard; 2],
    pub all_occupancy: Bitboard,
    pub white_to_move: bool,
    pub en_passant_target: Option<Square>,
    pub castling_rights: u8,
    pub hash: u64,
    pub halfmove_clock: u32,
    pub position_history: Vec<u64>,
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    /// Convert a 0-based square index (0..63) to a `Square` (rank, file).
    ///
    /// This helper is handy when iterating bitboards and converting
    /// trailing-zero indices into board coordinates.
    pub fn square_from_index(index: usize) -> Square {
        BitboardUtils::square_from_index(index)
    }

    #[allow(dead_code)]
    pub fn file_mask(file: usize) -> Bitboard {
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
    fn empty() -> Self {
        Board {
            bitboards: [[0; 6]; 2],
            occupancy: [0; 2],
            all_occupancy: 0,
            white_to_move: true,
            en_passant_target: None,
            castling_rights: 0,
            hash: 0,
            halfmove_clock: 0,
            position_history: Vec::new(),
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

    pub fn piece_at(&self, square: Square) -> Option<(Color, Piece)> {
        let mask = bitboard_for_square(square);
        for color_idx in 0..2 {
            if self.occupancy[color_idx] & mask != 0 {
                for piece_idx in 0..6 {
                    if self.bitboards[color_idx][piece_idx] & mask != 0 {
                        return Some((color_from_index(color_idx), piece_from_index(piece_idx)));
                    }
                }
            }
        }
        None
    }



    fn remove_piece_at(&mut self, square: Square) -> Option<(Color, Piece)> {
        let mask = bitboard_for_square(square);
        for color_idx in 0..2 {
            if self.occupancy[color_idx] & mask != 0 {
                for piece_idx in 0..6 {
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

    fn place_piece_at(&mut self, square: Square, piece: (Color, Piece)) {
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

    fn get_square(&self, rank: usize, file: usize) -> Option<(Color, Piece)> {
        self.piece_at(Square(rank, file))
    }

    fn set_square(
        &mut self,
        rank: usize,
        file: usize,
        piece: Option<(Color, Piece)>,
    ) -> Option<(Color, Piece)> {
        self.set_piece_at(Square(rank, file), piece)
    }

    fn has_castling_right(&self, color: Color, side: char) -> bool {
        let bit = castling_bit(color, side);
        bit != 0 && (self.castling_rights & bit) != 0
    }

    fn add_castling_right(&mut self, color: Color, side: char) {
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
            board.place_piece_at(Square(0, i), (Color::White, *piece));
            board.place_piece_at(Square(7, i), (Color::Black, *piece));
            board.place_piece_at(Square(1, i), (Color::White, Piece::Pawn));
            board.place_piece_at(Square(6, i), (Color::Black, Piece::Pawn));
        }
        board.hash = board.calculate_initial_hash(); // Calculate hash after setting up board
        board.halfmove_clock = 0;
        board.position_history.clear();
        board.position_history.push(board.hash);
        board
    }

    pub fn from_fen(fen: &str) -> Self {
        // Delegate to fallible parser and preserve previous panic behavior for callers
        match Board::try_from_fen(fen) {
            Ok(b) => b,
            Err(msg) => panic!("from_fen failed: {}", msg),
        }
    }

    /// Fallible FEN parser. Returns Ok(Board) or Err(String) with a message.
    pub fn try_from_fen(fen: &str) -> Result<Self, String> {
        let mut board = Board::empty();
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() < 4 {
            return Err("FEN must have at least 4 parts".to_string());
        }
        // Piece placement
        for (rank_idx, rank_str) in parts[0].split('/').enumerate() {
            let mut file = 0usize;
            for c in rank_str.chars() {
                if c.is_ascii_digit() {
                    file += c.to_digit(10).ok_or_else(|| "Invalid digit in FEN".to_string())? as usize;
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
                        _ => return Err(format!("Invalid piece char '{}' in FEN", c)),
                    };
                    board.place_piece_at(Square(7 - rank_idx, file), (color, piece));
                    file += 1;
                }
            }
        }
        board.white_to_move = match parts[1] {
            "w" => true,
            "b" => false,
            other => return Err(format!("Invalid color part in FEN: {}", other)),
        };
        for c in parts[2].chars() {
            match c {
                'K' => board.add_castling_right(Color::White, 'K'),
                'Q' => board.add_castling_right(Color::White, 'Q'),
                'k' => board.add_castling_right(Color::Black, 'K'),
                'q' => board.add_castling_right(Color::Black, 'Q'),
                '-' => (),
                other => return Err(format!("Invalid castling char in FEN: {}", other)),
            }
        }
        let en_passant_target = if parts[3] != "-" {
            let chars: Vec<char> = parts[3].chars().collect();
            if chars.len() == 2 {
                Some(Square(rank_to_index(chars[1]), file_to_index(chars[0])))
            } else {
                return Err("Invalid en-passant square in FEN".to_string());
            }
        } else {
            None
        };

        board.en_passant_target = en_passant_target;
        board.hash = board.calculate_initial_hash(); // Calculate hash after setting up board
        board.halfmove_clock = 0;
        board.position_history.clear();
        board.position_history.push(board.hash);
        Ok(board)
    }

    /// Calculate the Zobrist hash for the current board state from scratch.
    ///
    /// This recomputes the full zobrist hash by XOR-ing piece, side-to-move,
    /// castling and en-passant keys. It's used when a board is initialized or
    /// after a FEN is loaded to ensure the `hash` field matches the position.
    fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;

        for color_idx in 0..2 {
            for piece_idx in 0..6 {
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
        if self.has_castling_right(Color::White, 'K') {
            hash ^= ZOBRIST.castling_keys[0][0];
        }
        if self.has_castling_right(Color::White, 'Q') {
            hash ^= ZOBRIST.castling_keys[0][1];
        }
        if self.has_castling_right(Color::Black, 'K') {
            hash ^= ZOBRIST.castling_keys[1][0];
        }
        if self.has_castling_right(Color::Black, 'Q') {
            hash ^= ZOBRIST.castling_keys[1][1];
        }

        // En passant target
        if let Some(ep_square) = self.en_passant_target {
            hash ^= ZOBRIST.en_passant_keys[ep_square.1]; // XOR based on the file
        }

        hash
    }

    // --- Make/Unmake Logic ---

    /// Make a move on the board, updating internal state and returning the
    /// minimal snapshot required to restore the previous position.
    ///
    /// The returned `UnmakeInfo` must be passed to `unmake_move` to restore
    /// the board to its previous state. This function updates the zobrist
    /// `hash`, halfmove clock, castling rights, en-passant target and the
    /// internal piece bitboards.
    pub fn make_move(&mut self, m: &Move) -> UnmakeInfo {
        let mut current_hash = self.hash;
        let previous_hash = self.hash;
        let previous_halfmove_clock = self.halfmove_clock;
    let previous_history_len = self.position_history.len();
        let color = self.current_color();

        let previous_en_passant_target = self.en_passant_target;
        let previous_castling_rights = self.castling_rights;

        current_hash ^= ZOBRIST.black_to_move_key;

        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= ZOBRIST.en_passant_keys[old_ep.1];
        }

        let mut captured_piece_info: Option<(Color, Piece)> = None;
        if m.is_en_passant {
            let capture_row = if color == Color::White {
                m.to.0 - 1
            } else {
                m.to.0 + 1
            };
            let capture_sq = Square(capture_row, m.to.1);
            let capture_idx = square_to_zobrist_index(capture_sq);
            captured_piece_info = self.get_square(capture_row, m.to.1);
            self.set_square(capture_row, m.to.1, None);

            if let Some((cap_col, cap_piece)) = captured_piece_info {
                current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                    [color_to_zobrist_index(cap_col)][capture_idx];
            }
        } else if !m.is_castling {
            captured_piece_info = self.get_square(m.to.0, m.to.1);
            if let Some((cap_col, cap_piece)) = captured_piece_info {
                let capture_idx = square_to_zobrist_index(m.to);
                current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                    [color_to_zobrist_index(cap_col)][capture_idx];
            }
        }

        let moving_piece_info = self
            .get_square(m.from.0, m.from.1)
            .expect("make_move 'from' empty");
        let (moving_color, moving_piece) = moving_piece_info;
        let from_sq_idx = square_to_zobrist_index(m.from);
        let to_sq_idx = square_to_zobrist_index(m.to);

        current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(moving_piece)]
            [color_to_zobrist_index(moving_color)][from_sq_idx];

        self.set_square(m.from.0, m.from.1, None);

        if m.is_castling {
            self.set_square(m.to.0, m.to.1, Some((color, Piece::King)));
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::King)]
                [color_to_zobrist_index(color)][to_sq_idx];

            let (rook_from_f, rook_to_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_from_sq = Square(m.to.0, rook_from_f);
            let rook_to_sq = Square(m.to.0, rook_to_f);
            let rook_info = self
                .get_square(rook_from_sq.0, rook_from_sq.1)
                .expect("Castling without rook");
            self.set_square(rook_from_sq.0, rook_from_sq.1, None);
            self.set_square(rook_to_sq.0, rook_to_sq.1, Some(rook_info));

            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)]
                [color_to_zobrist_index(color)][square_to_zobrist_index(rook_from_sq)];
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)]
                [color_to_zobrist_index(color)][square_to_zobrist_index(rook_to_sq)];
        } else {
            let piece_to_place = if let Some(promoted_piece) = m.promotion {
                (color, promoted_piece)
            } else {
                moving_piece_info
            };
            self.set_square(m.to.0, m.to.1, Some(piece_to_place));
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(piece_to_place.1)]
                [color_to_zobrist_index(piece_to_place.0)][to_sq_idx];
        }

        self.en_passant_target = None;
        if moving_piece == Piece::Pawn && (m.from.0 as isize - m.to.0 as isize).abs() == 2 {
            let ep_row = (m.from.0 + m.to.0) / 2;
            let ep_sq = Square(ep_row, m.from.1);
            self.en_passant_target = Some(ep_sq);
            current_hash ^= ZOBRIST.en_passant_keys[ep_sq.1];
        }

        let mut new_castling_rights = self.castling_rights;
        let mut castle_hash_diff: u64 = 0;

        if moving_piece == Piece::King {
            if new_castling_rights & castling_bit(color, 'K') != 0 {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                new_castling_rights &= !castling_bit(color, 'K');
            }
            if new_castling_rights & castling_bit(color, 'Q') != 0 {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                new_castling_rights &= !castling_bit(color, 'Q');
            }
        } else if moving_piece == Piece::Rook {
            let start_rank = if color == Color::White { 0 } else { 7 };
            if m.from == Square(start_rank, 0)
                && new_castling_rights & castling_bit(color, 'Q') != 0
            {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                new_castling_rights &= !castling_bit(color, 'Q');
            } else if m.from == Square(start_rank, 7)
                && new_castling_rights & castling_bit(color, 'K') != 0
            {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                new_castling_rights &= !castling_bit(color, 'K');
            }
        }

        if let Some((captured_color, captured_piece)) = captured_piece_info {
            if captured_piece == Piece::Rook {
                let start_rank = if captured_color == Color::White { 0 } else { 7 };
                if m.to == Square(start_rank, 0)
                    && new_castling_rights & castling_bit(captured_color, 'Q') != 0
                {
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][1];
                    new_castling_rights &= !castling_bit(captured_color, 'Q');
                } else if m.to == Square(start_rank, 7)
                    && new_castling_rights & castling_bit(captured_color, 'K') != 0
                {
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][0];
                    new_castling_rights &= !castling_bit(captured_color, 'K');
                }
            }
        }
        self.castling_rights = new_castling_rights;
        current_hash ^= castle_hash_diff;

        self.white_to_move = !self.white_to_move;
        self.hash = current_hash;

        // Update halfmove clock
        if moving_piece == Piece::Pawn || captured_piece_info.is_some() {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        }

        // Update position history
        self.position_history.push(self.hash);

        UnmakeInfo {
            captured_piece_info,
            previous_en_passant_target,
            previous_castling_rights,
            previous_hash,
            previous_halfmove_clock,
            previous_history_len,
        }
    }

    /// Restore a previously-made move using the `UnmakeInfo` returned by
    /// `make_move`.
    ///
    /// This restores the board state (including `hash`, clocks and pieces)
    /// exactly to the state before the corresponding `make_move` call.
    pub fn unmake_move(&mut self, m: &Move, info: UnmakeInfo) {
        // Restore state directly from info
        self.white_to_move = !self.white_to_move; // Switch turn back first
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash; // Restore hash directly!

        // Restore halfmove clock and position history
        self.halfmove_clock = info.previous_halfmove_clock;
        self.position_history
            .truncate(info.previous_history_len);

        // Restore pieces on board (no hash updates needed here as hash is fully restored)
        let color = self.current_color();

        let piece_that_moved = if m.promotion.is_some() {
            (color, Piece::Pawn)
        } else if m.is_castling {
            (color, Piece::King) // Assume king if castling
        } else {
            self.get_square(m.to.0, m.to.1)
                .expect("Unmake move: 'to' square empty?")
        };

        if m.is_castling {
            self.set_square(m.from.0, m.from.1, Some(piece_that_moved));
            self.set_square(m.to.0, m.to.1, None);

            let (rook_orig_f, rook_moved_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) }; // KS or QS
            let rook_info = self
                .get_square(m.to.0, rook_moved_f)
                .expect("Unmake castling: rook missing");
            self.set_square(m.to.0, rook_moved_f, None);
            self.set_square(m.to.0, rook_orig_f, Some(rook_info));
        } else {
            self.set_square(m.from.0, m.from.1, Some(piece_that_moved));

            if m.is_en_passant {
                self.set_square(m.to.0, m.to.1, None);
                let capture_row = if color == Color::White {
                    m.to.0 - 1
                } else {
                    m.to.0 + 1
                };
                self.set_square(capture_row, m.to.1, info.captured_piece_info);
            } else {
                // Regular move: Put back captured piece (or None)
                self.set_square(m.to.0, m.to.1, info.captured_piece_info);
            }
        }
    }

    /// Make a "null move" (pass the turn) for null-move pruning.
    /// Returns a NullUnmake snapshot to restore position.
    pub fn make_null_move(&mut self) -> NullUnmake {
        let previous_en_passant_target = self.en_passant_target;
        let previous_hash = self.hash;
        let previous_halfmove_clock = self.halfmove_clock;
        let previous_history_len = self.position_history.len();

        // Update side to move and zobrist hash accordingly
        self.white_to_move = !self.white_to_move;
        self.hash ^= ZOBRIST.black_to_move_key;

        // Null move increments halfmove clock
        self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        self.en_passant_target = None;
        self.position_history.push(self.hash);

        NullUnmake {
            previous_en_passant_target,
            previous_hash,
            previous_halfmove_clock,
            previous_history_len,
        }
    }

    /// Restore state after a null move using the provided snapshot.
    pub fn unmake_null_move(&mut self, info: NullUnmake) {
        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.hash = info.previous_hash;
        self.halfmove_clock = info.previous_halfmove_clock;
        self.position_history.truncate(info.previous_history_len);
    }

    // --- Move Generation (largely unchanged logic, but uses new Move struct) ---
    // Provide "into" variants that accept a reusable buffer to avoid allocations.

    fn generate_pseudo_moves_into(&self, moves: &mut crate::core::types::MoveList) {
        crate::movegen::MoveGen::generate_pseudo_moves_into(self, moves);
    }

    // Removed unused `generate_pseudo_moves` (use `generate_pseudo_moves_into` to avoid allocation).

    #[allow(clippy::too_many_arguments)]


    // --- Check Detection (Refactored) ---

    // Finds the king of the specified color
    fn find_king(&self, color: Color) -> Option<Square> {
        let color_idx = color_to_zobrist_index(color);
        let king_bb = self.bitboards[color_idx][piece_to_zobrist_index(Piece::King)];
        if king_bb == 0 {
            None
        } else {
            let index = king_bb.trailing_zeros() as usize;
            Some(Self::square_from_index(index))
        }
    }

    // Checks if a square is attacked by the opponent WITHOUT cloning
    // Takes &self because it only reads the state
    fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        let color_idx = color_to_zobrist_index(attacker_color);
        let square_mask = bitboard_for_square(square);

        let pawns = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Pawn)];
        if attacker_color == Color::White {
            let attacks = ((pawns & BitboardUtils::NOT_FILE_H) << 9) | ((pawns & BitboardUtils::NOT_FILE_A) << 7);
            if attacks & square_mask != 0 {
                return true;
            }
        } else {
            let attacks = ((pawns & BitboardUtils::NOT_FILE_A) >> 9) | ((pawns & BitboardUtils::NOT_FILE_H) >> 7);
            if attacks & square_mask != 0 {
                return true;
            }
        }

        let knights = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Knight)];
        if Self::knight_attacks(square) & knights != 0 {
            return true;
        }

        let kings = self.bitboards[color_idx][piece_to_zobrist_index(Piece::King)];
        if Self::king_attacks(square) & kings != 0 {
            return true;
        }

        let bishop_like = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Bishop)]
            | self.bitboards[color_idx][piece_to_zobrist_index(Piece::Queen)];
        if Self::bishop_attacks(square, self.all_occupancy) & bishop_like != 0 {
            return true;
        }

        let rook_like = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Rook)]
            | self.bitboards[color_idx][piece_to_zobrist_index(Piece::Queen)];
        if Self::rook_attacks(square, self.all_occupancy) & rook_like != 0 {
            return true;
        }

        // No attackers found
        false
    }

    // Now takes &self
    pub(crate) fn is_in_check(&self, color: Color) -> bool {
        if let Some(king_sq) = self.find_king(color) {
            self.is_square_attacked(king_sq, self.opponent_color(color))
        } else {
            false // Or panic? King should always be on the board in a valid game.
        }
    }

    // Generates only fully legal moves, takes &mut self
    // Generates only fully legal moves, takes &mut self
    pub fn generate_moves_into(&mut self, out: &mut crate::core::types::MoveList) {
        // Use a temporary buffer for pseudo moves (caller may reuse `out` across calls)
        let mut pseudo: crate::core::types::MoveList = crate::core::types::MoveList::new();
        self.generate_pseudo_moves_into(&mut pseudo);

        out.clear();
        let current_color = self.current_color();
        let opponent_color = self.opponent_color(current_color);

    for m in pseudo.into_iter() {
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
                out.push(m); // If safe, it's a legal move
            }
            self.unmake_move(&m, info); // Unmake the move to restore state for next iteration
        }
    }

    // Removed unused allocation-creating wrapper `generate_moves`. Prefer `generate_moves_into`.

    // --- Game State Checks (need &mut self if they use generate_moves) ---

    // is_checkmate and is_stalemate now need &mut self
    // Removed unused `is_checkmate` and `is_stalemate` helpers. Use search drivers or tests instead.

    /// Returns true if the position is a draw by 50-move rule or threefold repetition
    pub fn is_draw(&self) -> bool {
        // 50-move rule: 100 half-moves without pawn move or capture
        if self.halfmove_clock >= 100 {
            return true;
        }
        // Threefold repetition: count occurrences of current hash in history
        let current_hash = self.hash;
        let occurrences = self
            .position_history
            .iter()
            .filter(|&&h| h == current_hash)
            .count();
        occurrences >= 3
    }



    // --- Search Functions (Refactored) ---

    fn negamax(
        &mut self,
        tt: &mut TranspositionTable, // Pass TT
        depth: u32,
        alpha: i32,
        beta: i32,
        moves_buf: &mut crate::core::types::MoveList,
    ) -> i32 {
        // Create a temporary ordering context for callers that don't provide one
        let mut ctx = crate::ordering::OrderingContext::new(256);
        crate::search::negamax(self, tt, depth, alpha, beta, moves_buf, &mut ctx)
    }

    // Quiescence search (also takes TT, but primarily for passing down)
    // Removed unused Board::quiesce wrapper; call crate::search::quiesce directly.

    // Run a single root depth search over `root_moves`. This encapsulates the loop
    // that iterates root moves, calls `negamax`, and returns the best move/score.
    // The `should_abort` closure is called before each move and may be used by
    // time-limited searches to abort mid-root.
    #[allow(dead_code)]


    // Allocation-returning tactical move generator removed; use `generate_tactical_moves_into`.

    pub fn generate_tactical_moves_into(&mut self, out: &mut crate::core::types::MoveList) {
        out.clear();
        let mut pseudo_tactical_moves: crate::core::types::MoveList = crate::core::types::MoveList::new();

        crate::movegen::MoveGen::generate_tactical_moves_into(self, &mut pseudo_tactical_moves);

        for m in pseudo_tactical_moves {
            if m.is_castling {
                continue;
            }

            let info = self.make_move(&m);
            if !self.is_in_check(self.current_color()) {
                out.push(m);
            }
            self.unmake_move(&m, info);
        }
    }

    // --- Perft (for testing, now takes &mut self) ---
    pub fn perft(&mut self, depth: usize) -> u64 {
        crate::perft::Perft::perft(self, depth)
    }

    // --- Utility Functions ---
    /// Returns the color whose turn it currently is.
    ///
    /// Convenience accessor: `Color::White` when `white_to_move` is true,
    /// otherwise `Color::Black`.
    pub(crate) fn current_color(&self) -> Color {
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

    // Add a print function for debugging and make it public so callers/tests can use it.
    pub fn print(&self) {
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let piece_char = match self.get_square(rank, file) {
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
            castling_str.push('-');
        }
        println!("Castling: {}", castling_str);
        println!("------------------------------------");
    }
} // end impl Board

// Parses a move in UCI format (e.g., "e2e4", "e7e8q")
#[allow(dead_code)]

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
