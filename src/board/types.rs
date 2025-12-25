use std::fmt;
use std::ops::Index;
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::error::SquareError;

pub(crate) fn file_to_index(file: char) -> usize {
    file as usize - ('a' as usize)
}

pub(crate) fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('0' as usize) - 1
}

pub(crate) fn bit_for_square(sq: Square) -> Bitboard {
    Bitboard(1u64 << sq.index().as_usize())
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

/// Castling rights represented as a bitmask
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CastlingRights(u8);

impl CastlingRights {
    /// No castling rights
    #[must_use]
    pub const fn none() -> Self {
        CastlingRights(0)
    }

    /// All castling rights (both sides can castle kingside and queenside)
    #[must_use]
    pub const fn all() -> Self {
        CastlingRights(CASTLE_WHITE_K | CASTLE_WHITE_Q | CASTLE_BLACK_K | CASTLE_BLACK_Q)
    }

    /// Check if a specific castling right is set
    #[inline]
    #[must_use]
    pub const fn has(self, color: Color, kingside: bool) -> bool {
        let bit = Self::bit_for(color, kingside);
        self.0 & bit != 0
    }

    /// Set a specific castling right
    #[inline]
    pub fn set(&mut self, color: Color, kingside: bool) {
        self.0 |= Self::bit_for(color, kingside);
    }

    /// Remove a specific castling right
    #[inline]
    pub fn remove(&mut self, color: Color, kingside: bool) {
        self.0 &= !Self::bit_for(color, kingside);
    }

    /// Get the raw bitmask value (for Zobrist hashing)
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// Create from raw bitmask value
    #[inline]
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        CastlingRights(value)
    }

    /// Get the bit for a specific castling right
    #[inline]
    const fn bit_for(color: Color, kingside: bool) -> u8 {
        match (color, kingside) {
            (Color::White, true) => CASTLE_WHITE_K,
            (Color::White, false) => CASTLE_WHITE_Q,
            (Color::Black, true) => CASTLE_BLACK_K,
            (Color::Black, false) => CASTLE_BLACK_Q,
        }
    }
}

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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Piece {
    #[inline]
    #[must_use]
    pub(crate) const fn index(self) -> usize {
        match self {
            Piece::Pawn => 0,
            Piece::Knight => 1,
            Piece::Bishop => 2,
            Piece::Rook => 3,
            Piece::Queen => 4,
            Piece::King => 5,
        }
    }

    /// Parse a piece from a lowercase character (p, n, b, r, q, k)
    #[must_use]
    pub fn from_char(c: char) -> Option<Piece> {
        match c.to_ascii_lowercase() {
            'p' => Some(Piece::Pawn),
            'n' => Some(Piece::Knight),
            'b' => Some(Piece::Bishop),
            'r' => Some(Piece::Rook),
            'q' => Some(Piece::Queen),
            'k' => Some(Piece::King),
            _ => None,
        }
    }

    /// Convert piece to lowercase character
    #[inline]
    #[must_use]
    pub const fn to_char(self) -> char {
        match self {
            Piece::Pawn => 'p',
            Piece::Knight => 'n',
            Piece::Bishop => 'b',
            Piece::Rook => 'r',
            Piece::Queen => 'q',
            Piece::King => 'k',
        }
    }

    /// Convert piece to character with case based on color (uppercase for White)
    #[inline]
    #[must_use]
    pub fn to_fen_char(self, color: Color) -> char {
        let c = self.to_char();
        if color == Color::White {
            c.to_ascii_uppercase()
        } else {
            c
        }
    }

    /// Get the standard material value in centipawns.
    ///
    /// Returns approximate values: Pawn=100, Knight=320, Bishop=330,
    /// Rook=500, Queen=900, King=20000 (effectively infinite).
    #[inline]
    #[must_use]
    pub const fn value(self) -> i32 {
        match self {
            Piece::Pawn => 100,
            Piece::Knight => 320,
            Piece::Bishop => 330,
            Piece::Rook => 500,
            Piece::Queen => 900,
            Piece::King => 20000,
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum Color {
    White,
    Black,
}

impl Color {
    #[inline]
    #[must_use]
    pub(crate) const fn index(self) -> usize {
        match self {
            Color::White => 0,
            Color::Black => 1,
        }
    }

    /// Returns the opposite color
    #[inline]
    #[must_use]
    pub(crate) const fn opponent(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SquareIdx(pub u8);

impl SquareIdx {
    #[inline]
    #[must_use]
    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Square(pub usize, pub usize); // (rank, file)

impl Square {
    /// Create a new square with bounds checking
    #[must_use]
    pub fn new(rank: usize, file: usize) -> Option<Self> {
        if rank < 8 && file < 8 {
            Some(Square(rank, file))
        } else {
            None
        }
    }

    /// Get the rank (0-7, where 0 = rank 1)
    #[inline]
    #[must_use]
    pub const fn rank(self) -> usize {
        self.0
    }

    /// Get the file (0-7, where 0 = file a)
    #[inline]
    #[must_use]
    pub const fn file(self) -> usize {
        self.1
    }

    /// Flip the square vertically (e.g., a1 <-> a8)
    #[inline]
    #[must_use]
    pub const fn flip_vertical(self) -> Self {
        Square(7 - self.0, self.1)
    }

    /// Flip the square horizontally (e.g., a1 <-> h1)
    #[inline]
    #[must_use]
    pub const fn flip_horizontal(self) -> Self {
        Square(self.0, 7 - self.1)
    }

    /// Get the square's index (0-63, a1=0, b1=1, ..., h8=63)
    #[inline]
    #[must_use]
    pub const fn as_index(self) -> usize {
        self.0 * 8 + self.1
    }

    /// Create a square from an index (0-63)
    #[must_use]
    pub const fn from_index_const(idx: usize) -> Self {
        Square(idx / 8, idx % 8)
    }

    #[inline]
    #[must_use]
    pub(crate) fn from_index(idx: SquareIdx) -> Self {
        let idx = idx.0 as usize;
        Square(idx / 8, idx % 8)
    }

    #[inline]
    #[must_use]
    pub(crate) const fn index(self) -> SquareIdx {
        SquareIdx((self.0 * 8 + self.1) as u8)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", (self.1 as u8 + b'a') as char, self.0 + 1)
    }
}

impl PartialOrd for Square {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Square {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare by index (a1=0, b1=1, ..., h8=63)
        self.index().0.cmp(&other.index().0)
    }
}

impl TryFrom<(usize, usize)> for Square {
    type Error = SquareError;

    fn try_from((rank, file): (usize, usize)) -> Result<Self, Self::Error> {
        if rank >= 8 {
            return Err(SquareError::RankOutOfBounds { rank });
        }
        if file >= 8 {
            return Err(SquareError::FileOutOfBounds { file });
        }
        Ok(Square(rank, file))
    }
}

impl FromStr for Square {
    type Err = SquareError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() != 2 {
            return Err(SquareError::InvalidNotation {
                notation: s.to_string(),
            });
        }

        let file = match chars[0] {
            'a'..='h' => chars[0] as usize - 'a' as usize,
            _ => {
                return Err(SquareError::InvalidNotation {
                    notation: s.to_string(),
                })
            }
        };

        let rank = match chars[1] {
            '1'..='8' => chars[1] as usize - '1' as usize,
            _ => {
                return Err(SquareError::InvalidNotation {
                    notation: s.to_string(),
                })
            }
        };

        Ok(Square(rank, file))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bitboard(pub u64);

// File masks (columns)
impl Bitboard {
    pub const FILE_A: Bitboard = Bitboard(0x0101010101010101);
    pub const FILE_B: Bitboard = Bitboard(0x0202020202020202);
    pub const FILE_C: Bitboard = Bitboard(0x0404040404040404);
    pub const FILE_D: Bitboard = Bitboard(0x0808080808080808);
    pub const FILE_E: Bitboard = Bitboard(0x1010101010101010);
    pub const FILE_F: Bitboard = Bitboard(0x2020202020202020);
    pub const FILE_G: Bitboard = Bitboard(0x4040404040404040);
    pub const FILE_H: Bitboard = Bitboard(0x8080808080808080);

    pub const RANK_1: Bitboard = Bitboard(0x00000000000000FF);
    pub const RANK_2: Bitboard = Bitboard(0x000000000000FF00);
    pub const RANK_3: Bitboard = Bitboard(0x0000000000FF0000);
    pub const RANK_4: Bitboard = Bitboard(0x00000000FF000000);
    pub const RANK_5: Bitboard = Bitboard(0x000000FF00000000);
    pub const RANK_6: Bitboard = Bitboard(0x0000FF0000000000);
    pub const RANK_7: Bitboard = Bitboard(0x00FF000000000000);
    pub const RANK_8: Bitboard = Bitboard(0xFF00000000000000);

    pub const EMPTY: Bitboard = Bitboard(0);
    pub const ALL: Bitboard = Bitboard(!0);

    /// Light squares (a1, c1, e1, g1, b2, d2, ...)
    pub const LIGHT_SQUARES: Bitboard = Bitboard(0x55AA55AA55AA55AA);
    /// Dark squares (b1, d1, f1, h1, a2, c2, ...)
    pub const DARK_SQUARES: Bitboard = Bitboard(0xAA55AA55AA55AA55);
}

impl Bitboard {
    /// Create a bitboard with a single square set
    #[inline]
    #[must_use]
    pub const fn from_square(sq: Square) -> Self {
        Bitboard(1 << (sq.0 * 8 + sq.1))
    }

    /// Returns an iterator over the square indices set in this bitboard
    #[inline]
    #[must_use]
    pub fn iter(self) -> BitboardIter {
        BitboardIter(self)
    }

    /// Returns true if the bitboard is empty
    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns the number of set bits (population count)
    #[inline]
    #[must_use]
    pub const fn popcount(self) -> u32 {
        self.0.count_ones()
    }

    /// Returns true if exactly one bit is set
    #[inline]
    #[must_use]
    pub const fn is_single(self) -> bool {
        self.0.is_power_of_two()
    }

    /// Returns true if the given square is set
    #[inline]
    #[must_use]
    pub const fn contains(self, sq: Square) -> bool {
        (self.0 & (1 << (sq.0 * 8 + sq.1))) != 0
    }

    /// Shift all bits north (toward rank 8)
    #[inline]
    #[must_use]
    pub const fn shift_north(self) -> Self {
        Bitboard(self.0 << 8)
    }

    /// Shift all bits south (toward rank 1)
    #[inline]
    #[must_use]
    pub const fn shift_south(self) -> Self {
        Bitboard(self.0 >> 8)
    }

    /// Shift all bits east (toward file h), masking off file a wraparound
    #[inline]
    #[must_use]
    pub const fn shift_east(self) -> Self {
        Bitboard((self.0 << 1) & !Self::FILE_A.0)
    }

    /// Shift all bits west (toward file a), masking off file h wraparound
    #[inline]
    #[must_use]
    pub const fn shift_west(self) -> Self {
        Bitboard((self.0 >> 1) & !Self::FILE_H.0)
    }

    /// Get the file mask for a given file index (0-7)
    #[inline]
    #[must_use]
    pub const fn file_mask(file: usize) -> Self {
        Bitboard(Self::FILE_A.0 << file)
    }

    /// Get the rank mask for a given rank index (0-7)
    #[inline]
    #[must_use]
    pub const fn rank_mask(rank: usize) -> Self {
        Bitboard(Self::RANK_1.0 << (rank * 8))
    }

    /// Bitwise AND
    #[inline]
    #[must_use]
    pub const fn and(self, other: Self) -> Self {
        Bitboard(self.0 & other.0)
    }

    /// Bitwise OR
    #[inline]
    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        Bitboard(self.0 | other.0)
    }

    /// Bitwise XOR
    #[inline]
    #[must_use]
    pub const fn xor(self, other: Self) -> Self {
        Bitboard(self.0 ^ other.0)
    }

    /// Bitwise NOT
    #[inline]
    #[must_use]
    pub const fn not(self) -> Self {
        Bitboard(!self.0)
    }
}

/// Iterator over set bits in a Bitboard
pub struct BitboardIter(Bitboard);

impl Iterator for BitboardIter {
    type Item = SquareIdx;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            None
        } else {
            Some(pop_lsb(&mut self.0))
        }
    }
}

// Move flags (4 bits, values 0-15)
const FLAG_QUIET: u16 = 0;
const FLAG_DOUBLE_PAWN: u16 = 1;
const FLAG_CASTLE_KINGSIDE: u16 = 2;
const FLAG_CASTLE_QUEENSIDE: u16 = 3;
const FLAG_CAPTURE: u16 = 4;
const FLAG_EN_PASSANT: u16 = 5;
// 6-7 reserved
const FLAG_PROMO_KNIGHT: u16 = 8;
const FLAG_PROMO_BISHOP: u16 = 9;
const FLAG_PROMO_ROOK: u16 = 10;
const FLAG_PROMO_QUEEN: u16 = 11;
const FLAG_PROMO_CAPTURE_KNIGHT: u16 = 12;
const FLAG_PROMO_CAPTURE_BISHOP: u16 = 13;
const FLAG_PROMO_CAPTURE_ROOK: u16 = 14;
const FLAG_PROMO_CAPTURE_QUEEN: u16 = 15;

/// Compact 16-bit move representation.
///
/// Encoding:
/// - bits 0-5:   from square (0-63)
/// - bits 6-11:  to square (0-63)
/// - bits 12-15: flags (move type)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Move(u16);

impl Move {
    /// Create a null/empty move (used for initialization)
    #[inline]
    #[must_use]
    pub const fn null() -> Self {
        Move(0)
    }

    /// Create a quiet move (no capture, no special flags)
    #[inline]
    #[must_use]
    pub const fn quiet(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_QUIET)
    }

    /// Create a capture move
    #[inline]
    #[must_use]
    pub const fn capture(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_CAPTURE)
    }

    /// Create a double pawn push move
    #[inline]
    #[must_use]
    pub const fn double_pawn_push(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_DOUBLE_PAWN)
    }

    /// Create an en passant capture
    #[inline]
    #[must_use]
    pub const fn en_passant(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_EN_PASSANT)
    }

    /// Create a kingside castle move
    #[inline]
    #[must_use]
    pub const fn castle_kingside(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_CASTLE_KINGSIDE)
    }

    /// Create a queenside castle move
    #[inline]
    #[must_use]
    pub const fn castle_queenside(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_CASTLE_QUEENSIDE)
    }

    /// Create a promotion move (non-capture)
    #[inline]
    #[must_use]
    pub const fn new_promotion(from: Square, to: Square, piece: Piece) -> Self {
        let flag = match piece {
            Piece::Knight => FLAG_PROMO_KNIGHT,
            Piece::Bishop => FLAG_PROMO_BISHOP,
            Piece::Rook => FLAG_PROMO_ROOK,
            _ => FLAG_PROMO_QUEEN, // Default to queen for invalid pieces
        };
        Move::with_flag(from, to, flag)
    }

    /// Create a promotion capture move
    #[inline]
    #[must_use]
    pub const fn new_promotion_capture(from: Square, to: Square, piece: Piece) -> Self {
        let flag = match piece {
            Piece::Knight => FLAG_PROMO_CAPTURE_KNIGHT,
            Piece::Bishop => FLAG_PROMO_CAPTURE_BISHOP,
            Piece::Rook => FLAG_PROMO_CAPTURE_ROOK,
            _ => FLAG_PROMO_CAPTURE_QUEEN, // Default to queen for invalid pieces
        };
        Move::with_flag(from, to, flag)
    }

    /// Create a move with a specific flag
    #[inline]
    const fn with_flag(from: Square, to: Square, flag: u16) -> Self {
        let from_idx = (from.0 * 8 + from.1) as u16;
        let to_idx = (to.0 * 8 + to.1) as u16;
        Move(from_idx | (to_idx << 6) | (flag << 12))
    }

    /// Get the source square
    #[inline]
    #[must_use]
    pub const fn from(self) -> Square {
        let idx = (self.0 & 0x3F) as usize;
        Square(idx / 8, idx % 8)
    }

    /// Get the destination square
    #[inline]
    #[must_use]
    pub const fn to(self) -> Square {
        let idx = ((self.0 >> 6) & 0x3F) as usize;
        Square(idx / 8, idx % 8)
    }

    /// Get the flag bits
    #[inline]
    const fn flag(self) -> u16 {
        self.0 >> 12
    }

    /// Returns true if this move captures a piece (including en passant)
    #[inline]
    #[must_use]
    pub const fn is_capture(self) -> bool {
        let f = self.flag();
        f == FLAG_CAPTURE || f == FLAG_EN_PASSANT || f >= FLAG_PROMO_CAPTURE_KNIGHT
    }

    /// Returns true if this move is en passant
    #[inline]
    #[must_use]
    pub const fn is_en_passant(self) -> bool {
        self.flag() == FLAG_EN_PASSANT
    }

    /// Returns true if this move is castling (kingside or queenside)
    #[inline]
    #[must_use]
    pub const fn is_castling(self) -> bool {
        let f = self.flag();
        f == FLAG_CASTLE_KINGSIDE || f == FLAG_CASTLE_QUEENSIDE
    }

    /// Returns true if this is kingside castling (O-O)
    #[inline]
    #[must_use]
    pub const fn is_castle_kingside(self) -> bool {
        self.flag() == FLAG_CASTLE_KINGSIDE
    }

    /// Returns true if this is queenside castling (O-O-O)
    #[inline]
    #[must_use]
    pub const fn is_castle_queenside(self) -> bool {
        self.flag() == FLAG_CASTLE_QUEENSIDE
    }

    /// Returns true if this move is a double pawn push
    #[inline]
    #[must_use]
    pub const fn is_double_pawn_push(self) -> bool {
        self.flag() == FLAG_DOUBLE_PAWN
    }

    /// Returns true if this move is a pawn promotion
    #[inline]
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        self.flag() >= FLAG_PROMO_KNIGHT
    }

    /// Get the promotion piece, if this is a promotion move
    #[inline]
    #[must_use]
    pub const fn promotion(self) -> Option<Piece> {
        match self.flag() {
            FLAG_PROMO_KNIGHT | FLAG_PROMO_CAPTURE_KNIGHT => Some(Piece::Knight),
            FLAG_PROMO_BISHOP | FLAG_PROMO_CAPTURE_BISHOP => Some(Piece::Bishop),
            FLAG_PROMO_ROOK | FLAG_PROMO_CAPTURE_ROOK => Some(Piece::Rook),
            FLAG_PROMO_QUEEN | FLAG_PROMO_CAPTURE_QUEEN => Some(Piece::Queen),
            _ => None,
        }
    }

    /// Returns true if this move is "quiet" (not a capture, promotion, or special move)
    #[inline]
    #[must_use]
    pub const fn is_quiet(self) -> bool {
        let f = self.flag();
        f == FLAG_QUIET || f == FLAG_DOUBLE_PAWN
    }

    /// Returns true if this move is tactical (capture or promotion)
    #[inline]
    #[must_use]
    pub const fn is_tactical(self) -> bool {
        self.is_capture() || self.is_promotion()
    }

    /// Get the raw 16-bit value (for hashing/storage)
    #[inline]
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Create from raw 16-bit value
    #[inline]
    #[must_use]
    pub const fn from_u16(value: u16) -> Self {
        Move(value)
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Move({}{}", self.from(), self.to())?;
        if let Some(promo) = self.promotion() {
            write!(f, "={}", promo.to_char().to_ascii_uppercase())?;
        }
        if self.is_capture() {
            write!(f, " cap")?;
        }
        if self.is_castling() {
            write!(f, " castle")?;
        }
        if self.is_en_passant() {
            write!(f, " ep")?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.from(), self.to())?;
        if let Some(promo) = self.promotion() {
            write!(f, "{}", promo.to_char())?;
        }
        Ok(())
    }
}

pub(crate) const MAX_MOVES: usize = 256;
pub(crate) const MAX_PLY: usize = 128;
pub(crate) const EMPTY_MOVE: Move = Move::null();

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

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub(crate) fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Move> {
        self.as_slice().iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Move> {
        self.as_mut_slice().iter_mut()
    }

    #[must_use]
    pub fn get(&self, idx: usize) -> Option<Move> {
        if idx < self.len {
            Some(self.moves[idx])
        } else {
            None
        }
    }

    #[must_use]
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

impl Default for MoveList {
    fn default() -> Self {
        MoveList::new()
    }
}

/// Owning iterator over moves in a `MoveList`
pub struct MoveListIntoIter {
    list: MoveList,
    idx: usize,
}

impl Iterator for MoveListIntoIter {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.list.len {
            let mv = self.list.moves[self.idx];
            self.idx += 1;
            Some(mv)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.list.len - self.idx;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for MoveListIntoIter {}

impl IntoIterator for MoveList {
    type Item = Move;
    type IntoIter = MoveListIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        MoveListIntoIter { list: self, idx: 0 }
    }
}

impl Index<usize> for MoveList {
    type Output = Move;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < self.len, "MoveList index {} out of bounds (len {})", idx, self.len);
        &self.moves[idx]
    }
}

