use super::super::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use super::super::{Bitboard, Board, Color, MoveList, Piece, Square};

impl Board {
    pub(crate) fn generate_king_moves(&self, from: Square) -> MoveList {
        let mut moves = MoveList::new();
        let color = self.current_color();
        let back_rank = if color == Color::White { 0 } else { 7 };
        let from_idx = from.index().as_usize();
        let own_occ = self.occupied[color.index()].0;
        let targets = Bitboard(KING_ATTACKS[from_idx] & !own_occ);

        for to_idx in targets.iter() {
            let to_sq = Square::from_index(to_idx);
            moves.push(self.create_move(from, to_sq, None, false, false, false));
        }

        if from == Square(back_rank, 4) {
            if self.has_castling_right(color, 'K')
                && self.is_empty(Square(back_rank, 5))
                && self.is_empty(Square(back_rank, 6))
                && self.piece_at(Square(back_rank, 7)) == Some((color, Piece::Rook))
            {
                let to_sq = Square(back_rank, 6);
                moves.push(self.create_move(from, to_sq, None, true, false, false));
            }
            if self.has_castling_right(color, 'Q')
                && self.is_empty(Square(back_rank, 1))
                && self.is_empty(Square(back_rank, 2))
                && self.is_empty(Square(back_rank, 3))
                && self.piece_at(Square(back_rank, 0)) == Some((color, Piece::Rook))
            {
                let to_sq = Square(back_rank, 2);
                moves.push(self.create_move(from, to_sq, None, true, false, false));
            }
        }

        moves
    }

    pub(crate) fn find_king(&self, color: Color) -> Option<Square> {
        self.pieces[color.index()][Piece::King.index()]
            .iter()
            .next()
            .map(Square::from_index)
    }

    pub(crate) fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        let target_idx = square.index().as_usize();
        let c_idx = attacker_color.index();

        let pawn_sources = if attacker_color == Color::White {
            PAWN_ATTACKS[Color::Black.index()][target_idx]
        } else {
            PAWN_ATTACKS[Color::White.index()][target_idx]
        };
        if self.pieces[c_idx][Piece::Pawn.index()].0 & pawn_sources != 0 {
            return true;
        }

        if self.pieces[c_idx][Piece::Knight.index()].0 & KNIGHT_ATTACKS[target_idx] != 0 {
            return true;
        }

        if self.pieces[c_idx][Piece::King.index()].0 & KING_ATTACKS[target_idx] != 0 {
            return true;
        }

        let rook_like = self.pieces[c_idx][Piece::Rook.index()].0
            | self.pieces[c_idx][Piece::Queen.index()].0;
        let bishop_like = self.pieces[c_idx][Piece::Bishop.index()].0
            | self.pieces[c_idx][Piece::Queen.index()].0;

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
