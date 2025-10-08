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
    pub const fn square_from_index(index: usize) -> Square {
        Square(index / 8, index % 8)
    }

    pub const fn file_mask(file: usize) -> Bitboard {
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

pub const fn castling_bit(color: Color, side: char) -> u8 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Square, square_index};

    #[test]
    fn test_square_from_index() {
        assert_eq!(BitboardUtils::square_from_index(0), Square(0, 0)); // a1
        assert_eq!(BitboardUtils::square_from_index(7), Square(0, 7)); // h1
        assert_eq!(BitboardUtils::square_from_index(8), Square(1, 0)); // a2
        assert_eq!(BitboardUtils::square_from_index(63), Square(7, 7)); // h8
    }

    #[test]
    fn test_file_mask() {
        assert_eq!(BitboardUtils::file_mask(0), BitboardUtils::FILE_A);
        assert_eq!(BitboardUtils::file_mask(1), BitboardUtils::FILE_B);
        assert_eq!(BitboardUtils::file_mask(7), BitboardUtils::FILE_H);
    }

    #[test]
    fn test_knight_attacks() {
        // Test center knight
        let e4 = Square(4, 4); // e4 is index 36
        let attacks = BitboardUtils::knight_attacks(e4);
        // Knight from e4 should attack: f7, d7, f5, d5, g6, c6, g2, c2
        let expected_squares = vec![
            Square(5, 6), Square(5, 2), // f7, d3
            Square(3, 6), Square(3, 2), // f5, d1
            Square(6, 5), Square(6, 3), // g6, c5
            Square(2, 5), Square(2, 3), // g2, c3
        ];
        for sq in expected_squares {
            assert!(attacks & (1u64 << square_index(sq)) != 0, "Missing attack on {:?}", sq);
        }
        assert_eq!(attacks.count_ones(), 8); // Center knight has 8 attacks
    }

    #[test]
    fn test_king_attacks() {
        // Test center king
        let e4 = Square(4, 4);
        let attacks = BitboardUtils::king_attacks(e4);
        // King from e4 should attack all 8 adjacent squares
        let expected_squares = vec![
            Square(3, 3), Square(3, 4), Square(3, 5), // d3, e3, f3
            Square(4, 3),              Square(4, 5), // d4,     f4
            Square(5, 3), Square(5, 4), Square(5, 5), // d5, e5, f5
        ];
        for sq in expected_squares {
            assert!(attacks & (1u64 << square_index(sq)) != 0, "Missing attack on {:?}", sq);
        }
        assert_eq!(attacks.count_ones(), 8);
    }

    #[test]
    fn test_corner_knight_attacks() {
        // Test corner knight (should have only 2 attacks)
        let a1 = Square(0, 0);
        let attacks = BitboardUtils::knight_attacks(a1);
        assert_eq!(attacks.count_ones(), 2); // Corner knight has 2 attacks
        // Should attack b3 and c2
        assert!(attacks & (1u64 << square_index(Square(2, 1))) != 0); // b3
        assert!(attacks & (1u64 << square_index(Square(1, 2))) != 0); // c2
    }

    #[test]
    fn test_castling_bit() {
        assert_eq!(castling_bit(Color::White, 'K'), CASTLE_WHITE_KINGSIDE);
        assert_eq!(castling_bit(Color::White, 'Q'), CASTLE_WHITE_QUEENSIDE);
        assert_eq!(castling_bit(Color::Black, 'K'), CASTLE_BLACK_KINGSIDE);
        assert_eq!(castling_bit(Color::Black, 'Q'), CASTLE_BLACK_QUEENSIDE);
        assert_eq!(castling_bit(Color::White, 'X'), 0);
    }

    #[test]
    fn test_piece_from_index() {
        assert_eq!(piece_from_index(0), Piece::Pawn);
        assert_eq!(piece_from_index(1), Piece::Knight);
        assert_eq!(piece_from_index(2), Piece::Bishop);
        assert_eq!(piece_from_index(3), Piece::Rook);
        assert_eq!(piece_from_index(4), Piece::Queen);
        assert_eq!(piece_from_index(5), Piece::King);
    }

    #[test]
    fn test_color_from_index() {
        assert_eq!(color_from_index(0), Color::White);
        assert_eq!(color_from_index(1), Color::Black);
    }

    #[test]
    fn test_file_constants() {
        // Test that file constants are correct
        assert_eq!(BitboardUtils::FILE_A.count_ones(), 8);
        assert_eq!(BitboardUtils::FILE_H.count_ones(), 8);
        assert_eq!(BitboardUtils::FILE_A & BitboardUtils::FILE_H, 0); // No overlap

        // Test NOT_FILE constants
        assert_eq!(BitboardUtils::NOT_FILE_A.count_ones(), 56);
        assert_eq!(BitboardUtils::NOT_FILE_H.count_ones(), 56);
        assert_eq!(BitboardUtils::NOT_FILE_AB.count_ones(), 48);
        assert_eq!(BitboardUtils::NOT_FILE_GH.count_ones(), 48);
    }
}