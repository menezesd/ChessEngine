pub(crate) fn file_to_index(file: char) -> usize {
    file as usize - ('a' as usize)
}

pub(crate) fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('0' as usize) - 1
}

pub(crate) fn bit_for_square(sq: Square) -> Bitboard {
    Bitboard(1u64 << sq.index().0)
}

pub(crate) fn pop_lsb(bb: &mut Bitboard) -> SquareIdx {
    let idx = bb.0.trailing_zeros() as u8;
    bb.0 &= bb.0 - 1;
    SquareIdx(idx)
}

pub(crate) const CASTLE_WHITE_K: u8 = 1 << 0;
pub(crate) const CASTLE_WHITE_Q: u8 = 1 << 1;
pub(crate) const CASTLE_BLACK_K: u8 = 1 << 2;
pub(crate) const CASTLE_BLACK_Q: u8 = 1 << 3;

pub(crate) fn castle_bit(color: Color, side: char) -> u8 {
    match (color, side) {
        (Color::White, 'K') => CASTLE_WHITE_K,
        (Color::White, 'Q') => CASTLE_WHITE_Q,
        (Color::Black, 'K') => CASTLE_BLACK_K,
        (Color::Black, 'Q') => CASTLE_BLACK_Q,
        _ => 0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Piece {
    pub(crate) fn index(self) -> usize {
        match self {
            Piece::Pawn => 0,
            Piece::Knight => 1,
            Piece::Bishop => 2,
            Piece::Rook => 3,
            Piece::Queen => 4,
            Piece::King => 5,
        }
    }
}

/// Promotion piece choices in order of typical preference (queen first)
pub(crate) const PROMOTION_PIECES: [Piece; 4] = [
    Piece::Queen,
    Piece::Rook,
    Piece::Bishop,
    Piece::Knight,
];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub(crate) fn index(self) -> usize {
        match self {
            Color::White => 0,
            Color::Black => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SquareIdx(pub u8);

impl SquareIdx {
    pub(crate) fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Square(pub usize, pub usize); // (rank, file)

impl Square {
    pub(crate) fn from_index(idx: SquareIdx) -> Self {
        let idx = idx.0 as usize;
        Square(idx / 8, idx % 8)
    }

    pub(crate) fn index(self) -> SquareIdx {
        SquareIdx((self.0 * 8 + self.1) as u8)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bitboard(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub is_castling: bool,
    pub is_en_passant: bool,
    pub promotion: Option<Piece>,
    pub captured_piece: Option<Piece>,
}

impl Move {
    /// Create a null move (used for testing)
    pub fn null() -> Self {
        Move {
            from: Square(0, 0),
            to: Square(0, 0),
            is_castling: false,
            is_en_passant: false,
            promotion: None,
            captured_piece: None,
        }
    }
}

pub(crate) const MAX_MOVES: usize = 256;
pub(crate) const MAX_PLY: usize = 128;
pub(crate) const EMPTY_MOVE: Move = Move {
    from: Square(0, 0),
    to: Square(0, 0),
    is_castling: false,
    is_en_passant: false,
    promotion: None,
    captured_piece: None,
};

#[derive(Clone, Debug)]
pub struct MoveList {
    moves: [Move; MAX_MOVES],
    len: usize,
}

impl MoveList {
    pub(crate) fn new() -> Self {
        MoveList {
            moves: [EMPTY_MOVE; MAX_MOVES],
            len: 0,
        }
    }

    pub(crate) fn push(&mut self, mv: Move) {
        self.moves[self.len] = mv;
        self.len += 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(crate) fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Move> {
        self.as_slice().iter()
    }

    pub fn get(&self, idx: usize) -> Option<Move> {
        if idx < self.len {
            Some(self.moves[idx])
        } else {
            None
        }
    }

    pub fn first(&self) -> Option<Move> {
        self.get(0)
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = std::slice::Iter<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a> IntoIterator for &'a mut MoveList {
    type Item = &'a mut Move;
    type IntoIter = std::slice::IterMut<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

pub fn format_square(sq: Square) -> String {
    format!("{}{}", (sq.1 as u8 + b'a') as char, sq.0 + 1)
}
