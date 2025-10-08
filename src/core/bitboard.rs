use crate::core::types::{Bitboard, Color, Piece, Square, square_index};
use once_cell::sync::Lazy;

static KNIGHT_ATTACKS: Lazy<[Bitboard; 64]> = Lazy::new(|| {
    let mut table = [0u64; 64];
    for (index, slot) in table.iter_mut().enumerate() {
        let bit = 1u64 << index;
        let mut attacks = 0u64;
        // Mask the source bit before shifting to avoid wrapping across files
        attacks |= (bit & BitboardUtils::NOT_FILE_H) << 17; // +2 rank, +1 file
        attacks |= (bit & BitboardUtils::NOT_FILE_A) << 15; // +2 rank, -1 file
        attacks |= (bit & BitboardUtils::NOT_FILE_GH) << 10; // +1 rank, +2 files
        attacks |= (bit & BitboardUtils::NOT_FILE_AB) << 6; // +1 rank, -2 files
        attacks |= (bit & BitboardUtils::NOT_FILE_A) >> 17; // -2 rank, -1 file
        attacks |= (bit & BitboardUtils::NOT_FILE_H) >> 15; // -2 rank, +1 file
        attacks |= (bit & BitboardUtils::NOT_FILE_AB) >> 10; // -1 rank, -2 files
        attacks |= (bit & BitboardUtils::NOT_FILE_GH) >> 6; // -1 rank, +2 files
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
        attacks |= (bit & BitboardUtils::NOT_FILE_H) << 1;
        attacks |= (bit & BitboardUtils::NOT_FILE_A) >> 1;
        attacks |= (bit & BitboardUtils::NOT_FILE_H) << 9;
        attacks |= (bit & BitboardUtils::NOT_FILE_A) << 7;
        attacks |= (bit & BitboardUtils::NOT_FILE_A) >> 9;
        attacks |= (bit & BitboardUtils::NOT_FILE_H) >> 7;
        *slot = attacks;
    }
    table
});

pub struct BitboardUtils;

impl BitboardUtils {
    pub const FILE_A: Bitboard = 0x0101010101010101;
    pub const FILE_B: Bitboard = 0x0202020202020202;
    pub const FILE_G: Bitboard = 0x4040404040404040;
    pub const FILE_H: Bitboard = 0x8080808080808080;
    pub const NOT_FILE_A: Bitboard = !Self::FILE_A;
    pub const NOT_FILE_H: Bitboard = !Self::FILE_H;
    pub const NOT_FILE_AB: Bitboard = !Self::FILE_A & !Self::FILE_B;
    pub const NOT_FILE_GH: Bitboard = !Self::FILE_G & !Self::FILE_H;

    /// Convert a 0-based square index (0..63) to a `Square` (rank, file).
    ///
    /// This helper is handy when iterating bitboards and converting
    /// trailing-zero indices into board coordinates.
    pub fn square_from_index(index: usize) -> Square {
        Square(index / 8, index % 8)
    }

    pub fn file_mask(file: usize) -> Bitboard {
        Self::FILE_A << file
    }

    pub fn knight_attacks(square: Square) -> Bitboard {
        KNIGHT_ATTACKS[square_index(square)]
    }

    pub fn king_attacks(square: Square) -> Bitboard {
        KING_ATTACKS[square_index(square)]
    }
}

pub const CASTLE_WHITE_KINGSIDE: u8 = 0b0001;
pub const CASTLE_WHITE_QUEENSIDE: u8 = 0b0010;
pub const CASTLE_BLACK_KINGSIDE: u8 = 0b0100;
pub const CASTLE_BLACK_QUEENSIDE: u8 = 0b1000;

pub fn castling_bit(color: Color, side: char) -> u8 {
    match (color, side) {
        (Color::White, 'K') => CASTLE_WHITE_KINGSIDE,
        (Color::White, 'Q') => CASTLE_WHITE_QUEENSIDE,
        (Color::Black, 'K') => CASTLE_BLACK_KINGSIDE,
        (Color::Black, 'Q') => CASTLE_BLACK_QUEENSIDE,
        _ => 0,
    }
}

pub fn piece_from_index(index: usize) -> Piece {
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

pub fn color_from_index(index: usize) -> Color {
    match index {
        0 => Color::White,
        1 => Color::Black,
        _ => unreachable!("invalid color index"),
    }
}