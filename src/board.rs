use std::time::{Duration, Instant};

use crate::constants::MATE_SCORE;
use crate::magic;
use crate::transposition_table::TranspositionTable;
use crate::types::{
    bitboard_for_square, file_to_index, format_square, rank_to_index, square_index, Bitboard,
    Color, Move, Piece, Square,
};
use crate::ordering;
use crate::uci_info;
use crate::zobrist::{
    color_to_zobrist_index, piece_to_zobrist_index, square_to_zobrist_index, ZOBRIST,
};
use once_cell::sync::Lazy;

static KNIGHT_ATTACKS: Lazy<[Bitboard; 64]> = Lazy::new(|| {
    let mut table = [0u64; 64];
    for (index, slot) in table.iter_mut().enumerate() {
        let bit = 1u64 << index;
        let mut attacks = 0u64;
        // Mask the source bit before shifting to avoid wrapping across files
        attacks |= (bit & Board::NOT_FILE_H) << 17; // +2 rank, +1 file
        attacks |= (bit & Board::NOT_FILE_A) << 15; // +2 rank, -1 file
        attacks |= (bit & Board::NOT_FILE_GH) << 10; // +1 rank, +2 files
        attacks |= (bit & Board::NOT_FILE_AB) << 6; // +1 rank, -2 files
        attacks |= (bit & Board::NOT_FILE_A) >> 17; // -2 rank, -1 file
        attacks |= (bit & Board::NOT_FILE_H) >> 15; // -2 rank, +1 file
        attacks |= (bit & Board::NOT_FILE_AB) >> 10; // -1 rank, -2 files
        attacks |= (bit & Board::NOT_FILE_GH) >> 6; // -1 rank, +2 files
        *slot = attacks;
    }
    table
});

static KING_ATTACKS: Lazy<[Bitboard; 64]> = Lazy::new(|| {
    let mut table = [0u64; 64];
    for (index, slot) in table.iter_mut().enumerate() {
        let bit = 1u64 << index;
        let mut attacks = 0u64;
        attacks |= bit << 8;
        attacks |= bit >> 8;
        attacks |= (bit & Board::NOT_FILE_H) << 1;
        attacks |= (bit & Board::NOT_FILE_A) >> 1;
        attacks |= (bit & Board::NOT_FILE_H) << 9;
        attacks |= (bit & Board::NOT_FILE_A) << 7;
        attacks |= (bit & Board::NOT_FILE_A) >> 9;
        attacks |= (bit & Board::NOT_FILE_H) >> 7;
        *slot = attacks;
    }
    table
});

const CASTLE_WHITE_KINGSIDE: u8 = 0b0001;
const CASTLE_WHITE_QUEENSIDE: u8 = 0b0010;
const CASTLE_BLACK_KINGSIDE: u8 = 0b0100;
const CASTLE_BLACK_QUEENSIDE: u8 = 0b1000;

fn castling_bit(color: Color, side: char) -> u8 {
    match (color, side) {
        (Color::White, 'K') => CASTLE_WHITE_KINGSIDE,
        (Color::White, 'Q') => CASTLE_WHITE_QUEENSIDE,
        (Color::Black, 'K') => CASTLE_BLACK_KINGSIDE,
        (Color::Black, 'Q') => CASTLE_BLACK_QUEENSIDE,
        _ => 0,
    }
}

fn piece_from_index(index: usize) -> Piece {
    match index {
        0 => Piece::Pawn,
        1 => Piece::Knight,
        2 => Piece::Bishop,
        3 => Piece::Rook,
        4 => Piece::Queen,
        5 => Piece::King,
        _ => unreachable!("invalid piece index"),
    }
}

fn color_from_index(index: usize) -> Color {
    match index {
        0 => Color::White,
        1 => Color::Black,
        _ => unreachable!("invalid color index"),
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

/// MVV-LVA (Most Valuable Victim - Least Valuable Attacker) score helper.
///
/// Returns a heuristic score favouring captures of high-value pieces with
/// low-value attackers. Used for move ordering in search.
pub fn mvv_lva_score(m: &Move, board: &Board) -> i32 {
    let victim = m.captured_piece;
    let attacker = board.piece_at(m.from).map(|(_c, p)| p);
    ordering::mvv_lva_score_by_values(victim, attacker)
}

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
    const FILE_A: Bitboard = 0x0101010101010101;
    const FILE_B: Bitboard = 0x0202020202020202;
    const FILE_G: Bitboard = 0x4040404040404040;
    const FILE_H: Bitboard = 0x8080808080808080;
    const NOT_FILE_A: Bitboard = !Self::FILE_A;
    const NOT_FILE_H: Bitboard = !Self::FILE_H;
    const NOT_FILE_AB: Bitboard = !Self::FILE_A & !Self::FILE_B;
    const NOT_FILE_GH: Bitboard = !Self::FILE_G & !Self::FILE_H;

    /// Convert a 0-based square index (0..63) to a `Square` (rank, file).
    ///
    /// This helper is handy when iterating bitboards and converting
    /// trailing-zero indices into board coordinates.
    pub fn square_from_index(index: usize) -> Square {
        Square(index / 8, index % 8)
    }

    #[allow(dead_code)]
    pub fn file_mask(file: usize) -> Bitboard {
        Self::FILE_A << file
    }

    // `file_mask_allow` was unused; removed to trim dead code. Use `Board::file_mask` instead.

    pub fn knight_attacks(square: Square) -> Bitboard {
        KNIGHT_ATTACKS[square_index(square)]
    }
    pub fn king_attacks(square: Square) -> Bitboard {
        KING_ATTACKS[square_index(square)]
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

    fn add_leaper_moves<F>(
        &self,
        color: Color,
        pieces: Bitboard,
        attack_fn: F,
        include_quiet: bool,
        moves: &mut crate::types::MoveList,
    ) where
        F: Fn(Square) -> Bitboard,
    {
        let opponent_idx = color_to_zobrist_index(self.opponent_color(color));
        for from_index in Self::bits_iter(pieces) {
            let from = Self::square_from_index(from_index);
            let attacks = attack_fn(from);

            if include_quiet {
                let quiet_targets = attacks & !self.all_occupancy;
                for to_index in Self::bits_iter(quiet_targets) {
                    let to = Self::square_from_index(to_index);
                    self.add_move(moves, color, from, to, None, false, false);
                }
            }

            let capture_targets = attacks & self.occupancy[opponent_idx];
            for to_index in Self::bits_iter(capture_targets) {
                let to = Self::square_from_index(to_index);
                self.add_move(moves, color, from, to, None, false, false);
            }
        }
    }

    fn add_sliding_moves<F>(
        &self,
        color: Color,
        pieces: Bitboard,
        attack_fn: F,
        include_quiet: bool,
        moves: &mut crate::types::MoveList,
    ) where
        F: Fn(Square, Bitboard) -> Bitboard,
    {
        let opponent_idx = color_to_zobrist_index(self.opponent_color(color));
        for from_index in Self::bits_iter(pieces) {
            let from = Self::square_from_index(from_index);
            let attacks = attack_fn(from, self.all_occupancy);

            if include_quiet {
                let quiet_targets = attacks & !self.all_occupancy;
                for to_index in Self::bits_iter(quiet_targets) {
                    let to = Self::square_from_index(to_index);
                    self.add_move(moves, color, from, to, None, false, false);
                }
            }

            let capture_targets = attacks & self.occupancy[opponent_idx];
            for to_index in Self::bits_iter(capture_targets) {
                let to = Self::square_from_index(to_index);
                self.add_move(moves, color, from, to, None, false, false);
            }
        }
    }

    fn add_pawn_tactical_moves(&self, color: Color, moves: &mut crate::types::MoveList) {
        let color_idx = color_to_zobrist_index(color);
        let opponent_idx = color_to_zobrist_index(self.opponent_color(color));
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };
        let pawns = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Pawn)];

        for from_index in Self::bits_iter(pawns) {
            let from = Self::square_from_index(from_index);

            let forward_rank = from.0 as isize + dir;
            if (0..8).contains(&forward_rank) {
                let forward_sq = Square(forward_rank as usize, from.1);
                if forward_sq.0 == promotion_rank {
                    let forward_mask = bitboard_for_square(forward_sq);
                    if self.all_occupancy & forward_mask == 0 {
                        for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                            self.add_move(
                                moves,
                                color,
                                from,
                                forward_sq,
                                Some(promo),
                                false,
                                false,
                            );
                        }
                    }
                }
            }

            let capture_rank = from.0 as isize + dir;
            if (0..8).contains(&capture_rank) {
                for df in [-1, 1] {
                    let capture_file = from.1 as isize + df;
                    if !(0..8).contains(&capture_file) {
                        continue;
                    }
                    let target_sq = Square(capture_rank as usize, capture_file as usize);
                    let target_mask = bitboard_for_square(target_sq);

                    if self.occupancy[opponent_idx] & target_mask != 0 {
                        if target_sq.0 == promotion_rank {
                            for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                                self.add_move(
                                    moves,
                                    color,
                                    from,
                                    target_sq,
                                    Some(promo),
                                    false,
                                    false,
                                );
                            }
                        } else {
                            self.add_move(moves, color, from, target_sq, None, false, false);
                        }
                    } else if Some(target_sq) == self.en_passant_target {
                        self.add_move(moves, color, from, target_sq, None, false, true);
                    }
                }
            }
        }
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

    fn generate_pseudo_moves_into(&self, moves: &mut crate::types::MoveList) {
        moves.clear();
        let color = self.current_color();
        self.generate_pawn_moves_for(color, moves);
        self.generate_knight_moves_for(color, moves);
        self.generate_bishop_moves_for(color, moves);
        self.generate_rook_moves_for(color, moves);
        self.generate_queen_moves_for(color, moves);
        self.generate_king_moves_for(color, moves);
    }

    // Removed unused `generate_pseudo_moves` (use `generate_pseudo_moves_into` to avoid allocation).

    #[allow(clippy::too_many_arguments)]
    fn add_move(
        &self,
        moves: &mut crate::types::MoveList,
        color: Color,
        from: Square,
        to: Square,
        promotion: Option<Piece>,
        is_castling: bool,
        is_en_passant: bool,
    ) {
        let captured_piece = if is_en_passant {
            Some(Piece::Pawn)
        } else if !is_castling {
            self.piece_at(to)
                .and_then(|(c, p)| if c != color { Some(p) } else { None })
        } else {
            None
        };

        moves.push(Move {
            from,
            to,
            promotion,
            is_castling,
            is_en_passant,
            captured_piece,
        });
    }

    fn generate_pawn_moves_for(&self, color: Color, moves: &mut crate::types::MoveList) {
        let color_idx = color_to_zobrist_index(color);
        let opponent_color = self.opponent_color(color);
        let opponent_idx = color_to_zobrist_index(opponent_color);
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let start_rank = if color == Color::White { 1 } else { 6 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        let mut pawns = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Pawn)];
        while pawns != 0 {
            let from_index = pawns.trailing_zeros() as usize;
            pawns &= pawns - 1;
            let from = Self::square_from_index(from_index);
            let forward_rank = from.0 as isize + dir;
            let forward_file = from.1 as isize;

            if (0..8).contains(&forward_rank) {
                let forward_sq = Square(forward_rank as usize, forward_file as usize);
                let forward_mask = bitboard_for_square(forward_sq);
                if self.all_occupancy & forward_mask == 0 {
                    if forward_sq.0 == promotion_rank {
                        for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                            self.add_move(
                                moves,
                                color,
                                from,
                                forward_sq,
                                Some(promo),
                                false,
                                false,
                            );
                        }
                    } else {
                        self.add_move(moves, color, from, forward_sq, None, false, false);
                        if from.0 == start_rank {
                            let double_rank = forward_rank + dir;
                            if (0..8).contains(&double_rank) {
                                let double_sq = Square(double_rank as usize, forward_file as usize);
                                let double_mask = bitboard_for_square(double_sq);
                                if self.all_occupancy & double_mask == 0 {
                                    self.add_move(
                                        moves, color, from, double_sq, None, false, false,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            let capture_rank = from.0 as isize + dir;
            if (0..8).contains(&capture_rank) {
                for df in [-1, 1] {
                    let capture_file = from.1 as isize + df;
                    if !(0..8).contains(&capture_file) {
                        continue;
                    }
                    let target_sq = Square(capture_rank as usize, capture_file as usize);
                    let target_mask = bitboard_for_square(target_sq);
                    if self.occupancy[opponent_idx] & target_mask != 0 {
                        if target_sq.0 == promotion_rank {
                            for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                                self.add_move(
                                    moves,
                                    color,
                                    from,
                                    target_sq,
                                    Some(promo),
                                    false,
                                    false,
                                );
                            }
                        } else {
                            self.add_move(moves, color, from, target_sq, None, false, false);
                        }
                    } else if Some(target_sq) == self.en_passant_target {
                        self.add_move(moves, color, from, target_sq, None, false, true);
                    }
                }
            }
        }
    }

    fn generate_knight_moves_for(&self, color: Color, moves: &mut crate::types::MoveList) {
        let color_idx = color_to_zobrist_index(color);
        let knights = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Knight)];
        self.add_leaper_moves(color, knights, Self::knight_attacks, true, moves);
    }

    fn generate_bishop_moves_for(&self, color: Color, moves: &mut crate::types::MoveList) {
        let color_idx = color_to_zobrist_index(color);
        let bishops = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Bishop)];
        self.add_sliding_moves(color, bishops, Self::bishop_attacks, true, moves);
    }

    fn generate_rook_moves_for(&self, color: Color, moves: &mut crate::types::MoveList) {
        let color_idx = color_to_zobrist_index(color);
        let rooks = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Rook)];
        self.add_sliding_moves(color, rooks, Self::rook_attacks, true, moves);
    }

    fn generate_queen_moves_for(&self, color: Color, moves: &mut crate::types::MoveList) {
        let color_idx = color_to_zobrist_index(color);
        let queens = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Queen)];
        self.add_sliding_moves(
            color,
            queens,
            |sq, occ| Self::rook_attacks(sq, occ) | Self::bishop_attacks(sq, occ),
            true,
            moves,
        );
    }

    fn generate_king_moves_for(&self, color: Color, moves: &mut crate::types::MoveList) {
        let color_idx = color_to_zobrist_index(color);
        let kings = self.bitboards[color_idx][piece_to_zobrist_index(Piece::King)];
        self.add_leaper_moves(color, kings, Self::king_attacks, true, moves);

        if kings == 0 {
            return;
        }

        let from_index = kings.trailing_zeros() as usize;
        let from = Self::square_from_index(from_index);
        let back_rank = if color == Color::White { 0 } else { 7 };

        if from == Square(back_rank, 4) {
            let king_side_path = [Square(back_rank, 5), Square(back_rank, 6)];
            let queen_side_path = [
                Square(back_rank, 1),
                Square(back_rank, 2),
                Square(back_rank, 3),
            ];

            if self.has_castling_right(color, 'K')
                && king_side_path
                    .iter()
                    .all(|sq| self.all_occupancy & bitboard_for_square(*sq) == 0)
                && (self.bitboards[color_idx][piece_to_zobrist_index(Piece::Rook)]
                    & bitboard_for_square(Square(back_rank, 7)))
                    != 0
            {
                self.add_move(moves, color, from, Square(back_rank, 6), None, true, false);
            }

            if self.has_castling_right(color, 'Q')
                && queen_side_path
                    .iter()
                    .all(|sq| self.all_occupancy & bitboard_for_square(*sq) == 0)
                && (self.bitboards[color_idx][piece_to_zobrist_index(Piece::Rook)]
                    & bitboard_for_square(Square(back_rank, 0)))
                    != 0
            {
                self.add_move(moves, color, from, Square(back_rank, 2), None, true, false);
            }
        }
    }

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
            let attacks = ((pawns & Self::NOT_FILE_H) << 9) | ((pawns & Self::NOT_FILE_A) << 7);
            if attacks & square_mask != 0 {
                return true;
            }
        } else {
            let attacks = ((pawns & Self::NOT_FILE_A) >> 9) | ((pawns & Self::NOT_FILE_H) >> 7);
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
    pub fn generate_moves_into(&mut self, out: &mut crate::types::MoveList) {
        // Use a temporary buffer for pseudo moves (caller may reuse `out` across calls)
        let mut pseudo: crate::types::MoveList = crate::types::MoveList::new();
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

    pub(crate) fn evaluate(&self) -> i32 {
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

        // ...existing code...

        // Count material and piece-square table contributions by iterating bitboards
        let mut mg_score: i32 = 0;
        let mut eg_score: i32 = 0;

        // Pawn file counts for pawn-structure heuristics
        let mut white_pawns_by_file = [0u32; 8];
        let mut black_pawns_by_file = [0u32; 8];

        // Bishop counts
        let white_bishops = self.bitboards[color_to_zobrist_index(Color::White)]
            [piece_to_zobrist_index(Piece::Bishop)];
        let black_bishops = self.bitboards[color_to_zobrist_index(Color::Black)]
            [piece_to_zobrist_index(Piece::Bishop)];
        let white_bishop_count = white_bishops.count_ones();
        let black_bishop_count = black_bishops.count_ones();

        // Iterate each color and piece type
        for color_idx in 0..2 {
            let color = color_from_index(color_idx);
            for piece_idx in 0..6 {
                let bb = self.bitboards[color_idx][piece_idx];
                if bb == 0 {
                    continue;
                }
                let piece_mg = MATERIAL_MG[piece_idx];
                let piece_eg = MATERIAL_EG[piece_idx];

                for sq in Self::bits_iter(bb) {

                    // Convert sq (0..63) to rank,file
                    let rank = sq / 8;
                    let file = sq % 8;

                    // PST index: white pieces are flipped vertically
                    let pst_idx = if color == Color::White {
                        (7 - rank) * 8 + file
                    } else {
                        rank * 8 + file
                    };

                    // Add for white, subtract for black so score is white minus black
                    if color == Color::White {
                        mg_score += piece_mg + PST_MG[piece_idx][pst_idx];
                        eg_score += piece_eg + PST_EG[piece_idx][pst_idx];
                    } else {
                        mg_score -= piece_mg + PST_MG[piece_idx][pst_idx];
                        eg_score -= piece_eg + PST_EG[piece_idx][pst_idx];
                    }

                    // Pawn file counting
                    if piece_idx == piece_to_zobrist_index(Piece::Pawn) {
                        if color == Color::White {
                            white_pawns_by_file[file] += 1;
                        } else {
                            black_pawns_by_file[file] += 1;
                        }
                    }
                }
            }
        }

        // Calculate game phase
        let total_material_mg: i32 = (0..6)
            .map(|idx| {
                let white_bb = self.bitboards[0][idx];
                let black_bb = self.bitboards[1][idx];
                let cnt = (white_bb.count_ones() + black_bb.count_ones()) as i32;
                cnt * MATERIAL_MG[idx]
            })
            .sum();
        let max_material = 2
            * (MATERIAL_MG[1] * 2
                + MATERIAL_MG[2] * 2
                + MATERIAL_MG[3] * 2
                + MATERIAL_MG[4]
                + MATERIAL_MG[0] * 8);
        let mut phase = (total_material_mg as f32) / (max_material as f32);
        phase = phase.clamp(0.0, 1.0);

        let position_score = (phase * mg_score as f32 + (1.0 - phase) * eg_score as f32) as i32;
        score += position_score;

        // Bishop pair bonus
        if white_bishop_count >= 2 {
            score += 30;
        }
        if black_bishop_count >= 2 {
            score -= 30;
        }

        // Rook on open/semi-open files and pawn-structure penalties
        for file in 0..8 {
            let fpawns = (white_pawns_by_file[file] + black_pawns_by_file[file]) as i32;

            // Rooks on file: iterate rook bitboards
            let white_rooks = self.bitboards[color_to_zobrist_index(Color::White)]
                [piece_to_zobrist_index(Piece::Rook)];
            let black_rooks = self.bitboards[color_to_zobrist_index(Color::Black)]
                [piece_to_zobrist_index(Piece::Rook)];
            let file_mask = Board::FILE_A << file; // FILE_n mask

            if white_rooks & file_mask != 0 {
                if fpawns == 0 {
                    score += 15;
                } else if black_pawns_by_file[file] == 0 {
                    score += 7;
                }
            }
            if black_rooks & file_mask != 0 {
                if fpawns == 0 {
                    score -= 15;
                } else if white_pawns_by_file[file] == 0 {
                    score -= 7;
                }
            }

            // Pawn structure: isolated / doubled / passed
            let wpf = white_pawns_by_file[file] as i32;
            let bpf = black_pawns_by_file[file] as i32;

            if wpf > 0 {
                let left = if file > 0 {
                    white_pawns_by_file[file - 1]
                } else {
                    0
                };
                let right = if file < 7 {
                    white_pawns_by_file[file + 1]
                } else {
                    0
                };
                if left == 0 && right == 0 {
                    score -= 12;
                }
                if wpf > 1 {
                    score -= 12 * (wpf - 1);
                }
            }
            if bpf > 0 {
                let left = if file > 0 {
                    black_pawns_by_file[file - 1]
                } else {
                    0
                };
                let right = if file < 7 {
                    black_pawns_by_file[file + 1]
                } else {
                    0
                };
                if left == 0 && right == 0 {
                    score += 12;
                }
                if bpf > 1 {
                    score += 12 * (bpf - 1);
                }
            }

            // Passed pawn detection (approximate): no enemy pawns in same or adjacent files ahead
            // For white: pawn ranks increase; for black: decrease
            // We'll scan each pawn on this file to decide passedness
            // White passed pawns
            let wpawns_on_file = self.bitboards[color_to_zobrist_index(Color::White)]
                [piece_to_zobrist_index(Piece::Pawn)]
                & file_mask;
            for sq in Self::bits_iter(wpawns_on_file) {
                let rank = sq / 8;
                // check black pawns ahead on same or adjacent files
                let bb = self.bitboards[color_to_zobrist_index(Color::Black)]
                    [piece_to_zobrist_index(Piece::Pawn)];
                let file_adj_mask = (Board::FILE_A << file)
                    | (if file > 0 { Board::FILE_A << (file - 1) } else { 0 })
                    | (if file < 7 { Board::FILE_A << (file + 1) } else { 0 });
                // all squares with index < rank*8
                let ahead_mask = if rank * 8 >= 64 {
                    u64::MAX
                } else {
                    (1u64 << (rank * 8)) - 1
                };
                let is_passed = (bb & file_adj_mask & ahead_mask) == 0;
                if is_passed {
                    let bonus = 10 + (7 - rank as i32) * 7;
                    score += bonus;
                }
            }

            // Black passed pawns
            let bpawns_on_file = self.bitboards[color_to_zobrist_index(Color::Black)]
                [piece_to_zobrist_index(Piece::Pawn)]
                & file_mask;
            for sq in Self::bits_iter(bpawns_on_file) {
                let rank = sq / 8;
                // check white pawns ahead on same or adjacent files
                let bb = self.bitboards[color_to_zobrist_index(Color::White)]
                    [piece_to_zobrist_index(Piece::Pawn)];
                let file_adj_mask = (Board::FILE_A << file)
                    | (if file > 0 { Board::FILE_A << (file - 1) } else { 0 })
                    | (if file < 7 { Board::FILE_A << (file + 1) } else { 0 });
                let ahead_mask = if (rank + 1) * 8 >= 64 {
                    0u64
                } else {
                    !((1u64 << ((rank + 1) * 8)) - 1)
                };
                let is_passed = (bb & file_adj_mask & ahead_mask) == 0;
                if is_passed {
                    let bonus = 10 + rank as i32 * 7;
                    score -= bonus;
                }
            }
        }

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
        alpha: i32,
        beta: i32,
        moves_buf: &mut crate::types::MoveList,
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
    pub(crate) fn run_root_search<F>(
        &mut self,
        tt: &mut TranspositionTable,
        depth: u32,
        root_moves: &mut [Move],
        mut should_abort: F,
        window: Option<(i32, i32)>,
    ) -> (Option<Move>, i32, bool)
    where
        F: FnMut() -> bool,
    {
        let mut best_move: Option<Move> = None;

        // Alpha/Beta window for root search
        let mut alpha = -MATE_SCORE * 2;
        let beta = MATE_SCORE * 2;
        let mut best_score = -MATE_SCORE * 2;

        // Allow TT to promote a suggested move to the front for ordering
    crate::search::apply_tt_move_hint(&mut root_moves[..], tt, self.hash);

    let mut mv_buf: crate::types::MoveList = crate::types::MoveList::new();
        // Create a persistent OrderingContext for the root search so killers/history persist
        let mut ordering_ctx = crate::ordering::OrderingContext::new(256);
    for (idx, m) in root_moves.iter().enumerate() {
            if should_abort() {
                return (None, 0, false); // aborted mid-root
            }
            let info = self.make_move(m);
            // Use the aspiration window for the first root move if provided
            let score = if idx == 0 {
                if let Some((w_alpha, w_beta)) = window {
                    -crate::search::negamax(self, tt, depth - 1, w_alpha, w_beta, &mut mv_buf, &mut ordering_ctx)
                } else {
                    -crate::search::negamax(self, tt, depth - 1, -beta, -alpha, &mut mv_buf, &mut ordering_ctx)
                }
            } else {
                -crate::search::negamax(self, tt, depth - 1, -beta, -alpha, &mut mv_buf, &mut ordering_ctx)
            };
            self.unmake_move(m, info);

            if score > best_score {
                best_score = score;
                best_move = Some(*m);
            }

            alpha = alpha.max(best_score);
        }

        (best_move, best_score, true)
    }

    // Allocation-returning tactical move generator removed; use `generate_tactical_moves_into`.

    pub fn generate_tactical_moves_into(&mut self, out: &mut crate::types::MoveList) {
        out.clear();
        let current_color = self.current_color();
        let color_idx = color_to_zobrist_index(current_color);
        let mut pseudo_tactical_moves: crate::types::MoveList = crate::types::MoveList::new();

        self.add_pawn_tactical_moves(current_color, &mut pseudo_tactical_moves);

        let knights = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Knight)];
        self.add_leaper_moves(
            current_color,
            knights,
            Self::knight_attacks,
            false,
            &mut pseudo_tactical_moves,
        );

        let bishops = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Bishop)];
        self.add_sliding_moves(
            current_color,
            bishops,
            Self::bishop_attacks,
            false,
            &mut pseudo_tactical_moves,
        );

        let rooks = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Rook)];
        self.add_sliding_moves(
            current_color,
            rooks,
            Self::rook_attacks,
            false,
            &mut pseudo_tactical_moves,
        );

        let queens = self.bitboards[color_idx][piece_to_zobrist_index(Piece::Queen)];
        self.add_sliding_moves(
            current_color,
            queens,
            |sq, occ| Self::rook_attacks(sq, occ) | Self::bishop_attacks(sq, occ),
            false,
            &mut pseudo_tactical_moves,
        );

        let kings = self.bitboards[color_idx][piece_to_zobrist_index(Piece::King)];
        self.add_leaper_moves(
            current_color,
            kings,
            Self::king_attacks,
            false,
            &mut pseudo_tactical_moves,
        );

        for m in pseudo_tactical_moves {
            if m.is_castling {
                continue;
            }

            let info = self.make_move(&m);
            if !self.is_in_check(current_color) {
                out.push(m);
            }
            self.unmake_move(&m, info);
        }
    }

    // --- Perft (for testing, now takes &mut self) ---
    pub fn perft(&mut self, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }

    let mut mvbuf: crate::types::MoveList = crate::types::MoveList::new();
    self.generate_moves_into(&mut mvbuf);
        if depth == 1 {
            return mvbuf.len() as u64;
        }

        let mut nodes = 0;
        for m in mvbuf {
            let info = self.make_move(&m);
            nodes += self.perft(depth - 1);
            self.unmake_move(&m, info);
        }

        nodes
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
// Needs the current board state to find the matching legal move object.
/// Parse a UCI move string (e.g. "e2e4", "e7e8q") and return the matching
/// legal `Move` if found.
///
/// This function needs `&mut Board` because it calls into move generation to
/// find the legal move that corresponds to the UCI string. Returns `None` if
/// the string is invalid or no legal move matches.
pub fn parse_uci_move(board: &mut Board, uci_string: &str) -> Option<Move> {
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
    let mut legal_moves: crate::types::MoveList = crate::types::MoveList::new();
    board.generate_moves_into(&mut legal_moves);

    for legal_move in legal_moves {
        if legal_move.from == from_sq && legal_move.to == to_sq {
            // Check for promotion match
            if legal_move.promotion == promotion_piece {
                // Found the move! Return a clone of it.
                return Some(legal_move);
            }
            // If no promotion specified by user AND move is not a promotion, it's a match
            else if promotion_piece.is_none() && legal_move.promotion.is_none() {
                return Some(legal_move);
            }
        }
    }

    None // No matching legal move found
}

#[allow(dead_code)]
pub fn find_best_move(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_depth: u32,
) -> Option<Move> {
    find_best_move_with_context(board, tt, max_depth, None, None, false)
}

/// Find the best move using iterative deepening with optional sinks and info publishing.
///
/// - `board`: the current position to search.
/// - `tt`: transposition table used for move ordering and PV extraction.
/// - `max_depth`: maximum depth to search.
/// - `sink`: optional Arc<Mutex<Option<Move>>> updated with intermediate best moves.
/// - `info_sender`: optional sender for structured UCI info messages.
/// - `_is_ponder`: set to true when this is a ponder search (used for UCI `ponder` info).
pub fn find_best_move_with_context(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_depth: u32,
    sink: Option<std::sync::Arc<std::sync::Mutex<Option<Move>>>>,
    info_sender: Option<std::sync::mpsc::Sender<uci_info::Info>>,
    _is_ponder: bool,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut _best_score = -MATE_SCORE * 2;

    let mut legal_moves: crate::types::MoveList = crate::types::MoveList::new();
    board.generate_moves_into(&mut legal_moves);
    if legal_moves.is_empty() {
        return None;
    }
    if legal_moves.len() == 1 {
        return Some(legal_moves[0]); // No need to search further
    }
    let mut root_moves = legal_moves; // Reuse for move ordering (moved instead of clone)

    // Helper to build a PV string from the transposition table starting at the current hash
    fn build_pv_string(tt: &TranspositionTable, start_hash: u64) -> String {
        let mut pv = Vec::new();
        if let Some(entry) = tt.probe(start_hash) {
            if let Some(mv) = entry.best_move {
                pv.push(mv);
            }
        }
        let pv_strs: Vec<String> = pv
            .iter()
            .map(|m| format!("{}{}", format_square(m.from), format_square(m.to)))
            .collect();
        pv_strs.join(" ")
    }

    // Iterative Deepening Loop
    let search_start = Instant::now();

    for depth in 1..=max_depth {
        let _depth_start = Instant::now();
        let _nodes_before = crate::search_control::get_node_count();
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

        // Temporary moves buffer to pass into recursive negamax/quiesce calls
    let mut mv_buf: crate::types::MoveList = crate::types::MoveList::new();
        for m in &root_moves {
            let info = board.make_move(m);
            let score = -board.negamax(tt, depth - 1, -beta, -alpha, &mut mv_buf);
            board.unmake_move(m, info);

            if score > current_best_score {
                current_best_score = score;
                current_best_move = Some(*m);
            }

            alpha = alpha.max(current_best_score);
        }

        if let Some(mv) = current_best_move {
            _best_score = current_best_score;
            best_move = Some(mv);

            // publish intermediate best move to sink if provided
            if let Some(ref s) = sink {
                let mut lock = match s.lock() {
                    Ok(g) => g,
                    Err(poisoned) => {
                        eprintln!("warning: sink mutex poisoned, recovering");
                        poisoned.into_inner()
                    }
                };
                *lock = best_move;
            }

            // Build structured Info and send to the info channel if present
            if let Some(ref sender) = info_sender {
                let nodes_after = crate::search_control::get_node_count();
                let nodes_total = nodes_after;
                let elapsed_ms = search_start.elapsed().as_millis();
                let nps = if elapsed_ms > 0 {
                    Some(((nodes_total as u128 * 1000) / elapsed_ms) as u64)
                } else {
                    None
                };
                let pv = build_pv_string(tt, board.hash);
                let mut info = uci_info::Info {
                    depth: Some(depth),
                    nodes: Some(nodes_total),
                    nps,
                    time_ms: Some(elapsed_ms),
                    score_cp: None,
                    score_mate: None,
                    pv: Some(pv.clone()),
                    seldepth: Some(depth),
                    ponder: None,
                };
                if _best_score.abs() > (MATE_SCORE / 2) {
                    let mate_in = (MATE_SCORE - _best_score.abs() + 1) / 2;
                    info.score_mate = Some(mate_in);
                } else {
                    info.score_cp = Some(_best_score);
                }
                // If the caller indicated we are pondering, include the best move as 'ponder'
                if _is_ponder {
                    if let Some(bm) = best_move {
                        info.ponder = Some(format!(
                            "{}{}",
                            format_square(bm.from),
                            format_square(bm.to)
                        ));
                    }
                }
                let _ = sender.send(info);
            }

            // Optional: reorder root_moves so best move is searched first in next iteration
            if let Some(pos) = root_moves.iter().position(|m| *m == mv) {
                root_moves.swap(0, pos);
            }
        }
    }

    best_move
}

// Removed several backward-compatible wrapper functions that were unused
// (find_best_move_with_sink, find_best_move_with_time, etc.). Prefer the
// explicit `find_best_move_with_context` and `find_best_move_with_time_context` APIs.

pub fn find_best_move_with_time_context(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_time: Duration,
    start_time: Instant,
    sink: Option<std::sync::Arc<std::sync::Mutex<Option<Move>>>>,
    info_sender: Option<std::sync::mpsc::Sender<uci_info::Info>>,
    _is_ponder: bool,
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
    let mut legal_moves: crate::types::MoveList = crate::types::MoveList::new();
    board.generate_moves_into(&mut legal_moves);

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

        // Temporary moves buffer reused for recursive calls
    let mut mv_buf: crate::types::MoveList = crate::types::MoveList::new();
        for m in &legal_moves {
            if start_time.elapsed() + SAFETY_MARGIN >= max_time {
                break;
            }

            let info = board.make_move(m);
            let score = -board.negamax(tt, depth - 1, -beta, -alpha, &mut mv_buf);
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
            // publish best move for this depth
            if let Some(ref s) = sink {
                let mut lock = match s.lock() {
                    Ok(g) => g,
                    Err(poisoned) => {
                        eprintln!("warning: sink mutex poisoned, recovering");
                        poisoned.into_inner()
                    }
                };
                *lock = best_move;
            }

            // Send structured Info via channel if available
            if let Some(ref sender) = info_sender {
                // Build PV by cloning board and following TT best moves
                fn build_pv_using_board(
                    orig: &Board,
                    tt: &TranspositionTable,
                    max_ply: usize,
                ) -> String {
                    let mut b = orig.clone();
                    let mut pv = Vec::new();
                    for _ in 0..max_ply {
                        if let Some(entry) = tt.probe(b.hash) {
                            if let Some(mv) = entry.best_move {
                                pv.push(mv);
                                let _info = b.make_move(&mv);
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    let pv_strs: Vec<String> = pv
                        .iter()
                        .map(|m| format!("{}{}", format_square(m.from), format_square(m.to)))
                        .collect();
                    pv_strs.join(" ")
                }

                let nodes_total = crate::search_control::get_node_count();
                let elapsed_ms = start_time.elapsed().as_millis();
                let nps = if elapsed_ms > 0 {
                    Some(((nodes_total as u128 * 1000) / elapsed_ms) as u64)
                } else {
                    None
                };
                let pv = build_pv_using_board(board, tt, 20);
                let mut info = uci_info::Info {
                    depth: Some(depth),
                    nodes: Some(nodes_total),
                    nps,
                    time_ms: Some(elapsed_ms),
                    score_cp: None,
                    score_mate: None,
                    pv: Some(pv),
                    seldepth: None,
                    ponder: None,
                };
                if best_score.abs() > (MATE_SCORE / 2) {
                    let mate_in = (MATE_SCORE - best_score.abs() + 1) / 2;
                    info.score_mate = Some(mate_in);
                } else {
                    info.score_cp = Some(best_score);
                }
                if _is_ponder {
                    if let Some(bm) = best_move {
                        info.ponder = Some(format!(
                            "{}{}",
                            format_square(bm.from),
                            format_square(bm.to)
                        ));
                    }
                }
                let _ = sender.send(info);
            }

            last_depth_time = depth_start.elapsed();
            depth += 1;
        } else {
            break;
        }
    }

    best_move
}

#[allow(dead_code)]
pub fn find_best_move_with_time_with_sink(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_time: Duration,
    start_time: Instant,
    sink: Option<std::sync::Arc<std::sync::Mutex<Option<Move>>>>,
    info_sender: Option<std::sync::mpsc::Sender<uci_info::Info>>,
    is_ponder: bool,
) -> Option<Move> {
    find_best_move_with_time_context(board, tt, max_time, start_time, sink, info_sender, is_ponder)
}

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
