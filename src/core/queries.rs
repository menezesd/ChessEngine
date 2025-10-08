use crate::core::board::Board;
use crate::core::types::{bitboard_for_square, Color, Piece, Square};
use crate::core::zobrist::{color_to_zobrist_index, piece_to_zobrist_index};
use crate::core::bitboard::BitboardUtils;

impl Board {
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

    pub fn piece_at(&self, square: Square) -> Option<(Color, Piece)> {
        let mask = bitboard_for_square(square);
        for color_idx in 0..2 {
            if self.occupancy[color_idx] & mask != 0 {
                for piece_idx in 0..6 {
                    if self.bitboards[color_idx][piece_idx] & mask != 0 {
                        return Some((crate::core::bitboard::color_from_index(color_idx), crate::core::bitboard::piece_from_index(piece_idx)));
                    }
                }
            }
        }
        None
    }

    pub fn get_square(&self, rank: usize, file: usize) -> Option<(Color, Piece)> {
        self.piece_at(Square(rank, file))
    }

    pub fn has_castling_right(&self, color: Color, side: char) -> bool {
        let bit = crate::core::bitboard::castling_bit(color, side);
        bit != 0 && (self.castling_rights & bit) != 0
    }

    /// Finds the king of the specified color
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

    pub(crate) fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
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
            self.is_square_attacked(king_sq, Self::opponent_color(color))
        } else {
            false // Or panic? King should always be on the board in a valid game.
        }
    }
}