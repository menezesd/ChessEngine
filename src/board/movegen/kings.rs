use super::super::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use super::super::{Bitboard, Board, Color, MoveList, Piece, Square};

impl Board {
    pub(crate) fn generate_king_moves(&self, from: Square) -> MoveList {
        let mut moves = MoveList::new();
        let color = self.side_to_move();
        let back_rank = color.back_rank();
        let from_idx = from.index();
        let own_occ = self.occupied_by(color).0;
        let targets = Bitboard(KING_ATTACKS[from_idx] & !own_occ);

        for to_idx in targets.iter() {
            let to_sq = to_idx;
            moves.push(self.create_simple_move(from, to_sq));
        }

        if from == Square::new(back_rank, 4) {
            if self.has_castling_right(color, 'K')
                && self.is_empty(Square::new(back_rank, 5))
                && self.is_empty(Square::new(back_rank, 6))
                && self.piece_at(Square::new(back_rank, 7)) == Some((color, Piece::Rook))
            {
                let to_sq = Square::new(back_rank, 6);
                moves.push(Self::create_castling_move(from, to_sq));
            }
            if self.has_castling_right(color, 'Q')
                && self.is_empty(Square::new(back_rank, 1))
                && self.is_empty(Square::new(back_rank, 2))
                && self.is_empty(Square::new(back_rank, 3))
                && self.piece_at(Square::new(back_rank, 0)) == Some((color, Piece::Rook))
            {
                let to_sq = Square::new(back_rank, 2);
                moves.push(Self::create_castling_move(from, to_sq));
            }
        }

        moves
    }

    /// Get the cached king square for a color.
    /// This is O(1) instead of iterating the bitboard.
    /// Returns Option for API compatibility with callers checking for illegal positions.
    #[inline]
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn find_king(&self, color: Color) -> Option<Square> {
        // Use cached king square - much faster than iterating bitboard
        Some(self.king_square[color.index()])
    }

    pub(crate) fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        let target_idx = square.index();

        let pawn_sources = if attacker_color == Color::White {
            PAWN_ATTACKS[Color::Black.index()][target_idx]
        } else {
            PAWN_ATTACKS[Color::White.index()][target_idx]
        };
        if self.pieces_of(attacker_color, Piece::Pawn).0 & pawn_sources != 0 {
            return true;
        }

        if self.pieces_of(attacker_color, Piece::Knight).0 & KNIGHT_ATTACKS[target_idx] != 0 {
            return true;
        }

        if self.pieces_of(attacker_color, Piece::King).0 & KING_ATTACKS[target_idx] != 0 {
            return true;
        }

        let rook_like = self.pieces_of(attacker_color, Piece::Rook).0
            | self.pieces_of(attacker_color, Piece::Queen).0;
        let bishop_like = self.pieces_of(attacker_color, Piece::Bishop).0
            | self.pieces_of(attacker_color, Piece::Queen).0;

        if slider_attacks(target_idx, self.all_occupied.0, false) & rook_like != 0 {
            return true;
        }
        if slider_attacks(target_idx, self.all_occupied.0, true) & bishop_like != 0 {
            return true;
        }

        false
    }

    pub(crate) fn is_in_check(&self, color: Color) -> bool {
        if let Some(king_sq) = self.find_king(color) {
            self.is_square_attacked(king_sq, color.opponent())
        } else {
            false
        }
    }
}
