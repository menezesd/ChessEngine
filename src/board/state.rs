use std::collections::HashMap;

use super::{color_index, format_square, piece_index, Bitboard, Color, Piece, Square, CASTLE_BLACK_K,
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
pub struct Board {
    pub(crate) pieces: [[Bitboard; 6]; 2],
    pub(crate) occupied: [Bitboard; 2],
    pub(crate) all_occupied: Bitboard,
    pub(crate) white_to_move: bool,
    pub(crate) en_passant_target: Option<Square>,
    pub(crate) castling_rights: u8, // bitmask
    pub(crate) hash: u64,           // Zobrist hash
    pub(crate) halfmove_clock: u32,
    pub(crate) repetition_counts: HashMap<u64, u32>,
}

impl Board {
    pub fn print(&self) {
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let piece_char = match self.piece_at(Square(rank, file)) {
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
        println!("Turn: {}", if self.white_to_move { "White" } else { "Black" });
        if let Some(ep_target) = self.en_passant_target {
            println!("EP Target: {}", format_square(ep_target));
        }
        println!("Castling mask: {:#06b}", self.castling_rights);
        println!("------------------------------------");
    }

    pub fn debug_bitboards(&self) {
        let colors = [Color::White, Color::Black];
        let pieces = [
            (Piece::Pawn, "P"),
            (Piece::Knight, "N"),
            (Piece::Bishop, "B"),
            (Piece::Rook, "R"),
            (Piece::Queen, "Q"),
            (Piece::King, "K"),
        ];

        println!(
            "Side to move: {}",
            if self.white_to_move { "White" } else { "Black" }
        );
        println!("Castling mask: {:#06b}", self.castling_rights);
        if let Some(ep_target) = self.en_passant_target {
            println!("EP Target: {}", format_square(ep_target));
        }
        println!("All occupied: {:#018x}", self.all_occupied.0);

        for color in colors {
            let label = if color == Color::White { "White" } else { "Black" };
            for (piece, name) in pieces {
                let bb = self.pieces[color_index(color)][piece_index(piece)].0;
                println!("{} {}: {:#018x}", label, name, bb);
            }
        }
        println!("------------------------------------");
    }

    pub fn print_bitboard_grid(&self, label: &str, bb: Bitboard) {
        println!("{} {:#018x}", label, bb.0);
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let idx = (rank * 8 + file) as u8;
                let ch = if (bb.0 >> idx) & 1 == 1 { '1' } else { '.' };
                print!(" {} |", ch);
            }
            println!("\n  +---+---+---+---+---+---+---+---+");
        }
        println!("    a   b   c   d   e   f   g   h");
        println!("------------------------------------");
    }

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
        board.repetition_counts.insert(board.hash, 1);
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
            repetition_counts: HashMap::new(),
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
        self.repetition_counts.get(&self.hash).copied().unwrap_or(0) >= 3
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

fn bishops_all_same_color(bishops: u64) -> bool {
    let light_squares: u64 = 0x55AA55AA55AA55AA;
    let dark_squares: u64 = 0xAA55AA55AA55AA55;

    (bishops & light_squares == 0) || (bishops & dark_squares == 0)
}
