use super::super::attack_tables::{
    bishop_attacks, diagonal_rays, rook_attacks, slider_attacks, straight_rays, KING_ATTACKS,
    KNIGHT_ATTACKS, PAWN_ATTACKS,
};
use super::super::types::{Bitboard, Color, Move, MoveList, Piece, Square};
use super::super::Board;

impl Board {
    pub(super) fn generate_king_moves(&self, from: Square, moves: &mut MoveList) {
        let color = self.side_to_move();
        let back_rank = color.back_rank();
        self.generate_piece_steps(from, Piece::King, !self.occupied_by(color).0, moves);

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
    }

    /// Get the cached king square for a color.
    /// This is O(1) instead of iterating the bitboard.
    #[inline]
    pub(crate) fn find_king(&self, color: Color) -> Square {
        self.king_square[color.index()]
    }

    pub(crate) fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        self.is_square_attacked_with_occupancy(square, attacker_color, self.all_occupied.0, 0)
    }

    /// Test attacks after a hypothetical move. A captured piece is still in
    /// the piece bitboards, so mask it out independently of occupancy.
    pub(in crate::board) fn is_square_attacked_with_occupancy(
        &self,
        square: Square,
        attacker_color: Color,
        occupancy: u64,
        removed_attackers: u64,
    ) -> bool {
        let target_idx = square.index();
        let remaining = !removed_attackers;
        if self.is_attacked_by_leaper(target_idx, attacker_color, remaining) {
            return true;
        }

        let queens = self.pieces_of(attacker_color, Piece::Queen).0;
        let rook_like = (self.pieces_of(attacker_color, Piece::Rook).0 | queens) & remaining;
        if rook_like & straight_rays(target_idx) != 0
            && rook_attacks(target_idx, occupancy) & rook_like != 0
        {
            return true;
        }

        let bishop_like = (self.pieces_of(attacker_color, Piece::Bishop).0 | queens) & remaining;
        bishop_like & diagonal_rays(target_idx) != 0
            && bishop_attacks(target_idx, occupancy) & bishop_like != 0
    }

    pub(super) fn is_attacked_by_leaper(
        &self,
        target_idx: usize,
        attacker_color: Color,
        remaining: u64,
    ) -> bool {
        let pawn_sources = PAWN_ATTACKS[attacker_color.opponent().index()][target_idx];
        self.pieces_of(attacker_color, Piece::Pawn).0 & pawn_sources & remaining != 0
            || self.pieces_of(attacker_color, Piece::Knight).0
                & KNIGHT_ATTACKS[target_idx]
                & remaining
                != 0
            || self.pieces_of(attacker_color, Piece::King).0 & KING_ATTACKS[target_idx] & remaining
                != 0
    }

    pub(crate) fn is_in_check(&self, color: Color) -> bool {
        self.is_square_attacked(self.find_king(color), color.opponent())
    }

    /// Test a legal, non-promoting capture for direct or discovered check
    /// without changing the board.
    pub(in crate::board) fn capture_gives_check(&self, mv: Move, piece: Piece) -> bool {
        debug_assert!(mv.is_capture() && !mv.is_promotion());
        let color = self.side_to_move();
        let king = self.find_king(color.opponent());
        let from = Bitboard::from_square(mv.from()).0;
        let to = Bitboard::from_square(mv.to()).0;
        let mut occupancy = (self.all_occupied.0 & !from) | to;
        if mv.is_en_passant() {
            occupancy &= !Bitboard::from_square(mv.capture_square()).0;
        }

        let index = king.index();
        let direct = match piece {
            Piece::Pawn => PAWN_ATTACKS[color.opponent().index()][index],
            Piece::Knight => KNIGHT_ATTACKS[index],
            Piece::Bishop => slider_attacks(index, occupancy, true),
            Piece::Rook => slider_attacks(index, occupancy, false),
            Piece::Queen => {
                slider_attacks(index, occupancy, true) | slider_attacks(index, occupancy, false)
            }
            Piece::King => KING_ATTACKS[index],
        };
        direct & to != 0 || self.is_square_attacked_with_occupancy(king, color, occupancy, from)
    }
}
