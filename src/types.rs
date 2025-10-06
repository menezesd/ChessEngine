pub type Bitboard = u64;

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
pub struct Square(pub usize, pub usize);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub is_castling: bool,
    pub is_en_passant: bool,
    pub promotion: Option<Piece>,
    pub captured_piece: Option<Piece>,
}

pub fn file_to_index(file: char) -> usize {
    file as usize - ('a' as usize)
}

pub fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('0' as usize) - 1
}

pub fn format_square(sq: Square) -> String {
    format!("{}{}", (sq.1 as u8 + b'a') as char, sq.0 + 1)
}

pub fn square_index(sq: Square) -> usize {
    sq.0 * 8 + sq.1
}

pub fn bitboard_for_square(sq: Square) -> Bitboard {
    1u64 << square_index(sq)
}
