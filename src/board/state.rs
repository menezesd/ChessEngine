use std::collections::HashMap;

use super::{color_index, piece_index, Bitboard, Color, Piece, Square, CASTLE_BLACK_K,
    CASTLE_BLACK_Q, CASTLE_WHITE_K, CASTLE_WHITE_Q};

#[derive(Clone, Debug)]
pub struct UnmakeInfo {
    pub(crate) captured_piece_info: Option<(Color, Piece)>,
    pub(crate) previous_en_passant_target: Option<Square>,
    pub(crate) previous_castling_rights: u8,
    pub(crate) previous_hash: u64,
    pub(crate) previous_halfmove_clock: u32,
    pub(crate) made_hash: u64,
    pub(crate) previous_repetition_count: u32,
}

pub struct NullMoveInfo {
    pub(crate) previous_en_passant_target: Option<Square>,
    pub(crate) previous_hash: u64,
}

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

#[derive(Clone, Debug)]
pub struct Board {
    pub(crate) pieces: [[Bitboard; 6]; 2],
    pub(crate) occupied: [Bitboard; 2],
    pub(crate) all_occupied: Bitboard,
    pub(crate) white_to_move: bool,
    pub(crate) en_passant_target: Option<Square>,
    pub(crate) castling_rights: u8, // bitmask
    pub(crate) hash: u64,           // Zobrist hash
    pub(crate) halfmove_clock: u32,
    pub(crate) repetition_counts: RepetitionTable,
}

impl Board {
    pub fn new() -> Self {
        let mut board = Board::empty();
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
            board.set_piece(Square(0, i), Color::White, *piece);
            board.set_piece(Square(7, i), Color::Black, *piece);
            board.set_piece(Square(1, i), Color::White, Piece::Pawn);
            board.set_piece(Square(6, i), Color::Black, Piece::Pawn);
        }

        board.castling_rights =
            CASTLE_WHITE_K | CASTLE_WHITE_Q | CASTLE_BLACK_K | CASTLE_BLACK_Q;
        board.white_to_move = true;
        board.hash = board.calculate_initial_hash();
        board.repetition_counts.set(board.hash, 1);
        board
    }

    pub(crate) fn empty() -> Self {
        Board {
            pieces: [[Bitboard(0); 6]; 2],
            occupied: [Bitboard(0); 2],
            all_occupied: Bitboard(0),
            white_to_move: true,
            en_passant_target: None,
            castling_rights: 0,
            hash: 0,
            halfmove_clock: 0,
            repetition_counts: RepetitionTable::new(),
        }
    }

    pub fn hash(&self) -> u64 {
        self.hash
    }

    pub fn white_to_move(&self) -> bool {
        self.white_to_move
    }

    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove_clock
    }

    pub fn is_draw(&self) -> bool {
        if self.halfmove_clock >= 100 {
            return true;
        }
        self.repetition_counts.get(self.hash) >= 3
    }

    pub fn is_theoretical_draw(&self) -> bool {
        self.is_draw() || self.is_insufficient_material()
    }

    fn is_insufficient_material(&self) -> bool {
        let white = color_index(Color::White);
        let black = color_index(Color::Black);

        let pawns = self.pieces[white][piece_index(Piece::Pawn)].0
            | self.pieces[black][piece_index(Piece::Pawn)].0;
        let rooks = self.pieces[white][piece_index(Piece::Rook)].0
            | self.pieces[black][piece_index(Piece::Rook)].0;
        let queens = self.pieces[white][piece_index(Piece::Queen)].0
            | self.pieces[black][piece_index(Piece::Queen)].0;

        if pawns != 0 || rooks != 0 || queens != 0 {
            return false;
        }

        let white_knights =
            self.pieces[white][piece_index(Piece::Knight)].0.count_ones();
        let black_knights =
            self.pieces[black][piece_index(Piece::Knight)].0.count_ones();
        let white_bishops =
            self.pieces[white][piece_index(Piece::Bishop)].0.count_ones();
        let black_bishops =
            self.pieces[black][piece_index(Piece::Bishop)].0.count_ones();

        let total_minors = white_knights + black_knights + white_bishops + black_bishops;

        if total_minors == 0 || total_minors == 1 {
            return true;
        }

        let total_knights = white_knights + black_knights;
        let total_bishops = white_bishops + black_bishops;

        if total_knights == 0 && total_bishops == 2 {
            return bishops_all_same_color(self.pieces[white][piece_index(Piece::Bishop)].0
                | self.pieces[black][piece_index(Piece::Bishop)].0);
        }

        false
    }
}

impl Default for Board {
    fn default() -> Self {
        Board::new()
    }
}

fn bishops_all_same_color(bishops: u64) -> bool {
    let light_squares: u64 = 0x55AA55AA55AA55AA;
    let dark_squares: u64 = 0xAA55AA55AA55AA55;

    (bishops & light_squares == 0) || (bishops & dark_squares == 0)
}
