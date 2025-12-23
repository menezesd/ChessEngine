use super::super::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use super::super::{
    color_index, piece_index, pop_lsb, square_from_index, square_index, Bitboard, Board, Color,
    MoveList, Piece, Square,
};

impl Board {
    pub(crate) fn generate_king_moves(&self, from: Square) -> MoveList {
        let mut moves = MoveList::new();
        let color = self.current_color();
        let back_rank = if color == Color::White { 0 } else { 7 };
        let from_idx = square_index(from).as_usize();
        let own_occ = self.occupied[color_index(color)].0;
        let mut targets = Bitboard(KING_ATTACKS[from_idx] & !own_occ);

        while targets.0 != 0 {
            let to_idx = pop_lsb(&mut targets);
            let to_sq = square_from_index(to_idx);
            moves.push(self.create_move(from, to_sq, None, false, false));
        }

        if from == Square(back_rank, 4) {
            if self.has_castling_right(color, 'K')
                && self.is_empty(Square(back_rank, 5))
                && self.is_empty(Square(back_rank, 6))
                && self.piece_at(Square(back_rank, 7)) == Some((color, Piece::Rook))
            {
                let to_sq = Square(back_rank, 6);
                moves.push(self.create_move(from, to_sq, None, true, false));
            }
            if self.has_castling_right(color, 'Q')
                && self.is_empty(Square(back_rank, 1))
                && self.is_empty(Square(back_rank, 2))
                && self.is_empty(Square(back_rank, 3))
                && self.piece_at(Square(back_rank, 0)) == Some((color, Piece::Rook))
            {
                let to_sq = Square(back_rank, 2);
                moves.push(self.create_move(from, to_sq, None, true, false));
            }
        }

        moves
    }

    pub(crate) fn find_king(&self, color: Color) -> Option<Square> {
        for r in 0..8 {
            for f in 0..8 {
                let sq = Square(r, f);
                if self.piece_at(sq) == Some((color, Piece::King)) {
                    return Some(sq);
                }
            }
        }
        None
    }

    pub(crate) fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        let target_idx = square_index(square).as_usize();
        let c_idx = color_index(attacker_color);

        let pawn_sources = if attacker_color == Color::White {
            PAWN_ATTACKS[color_index(Color::Black)][target_idx]
        } else {
            PAWN_ATTACKS[color_index(Color::White)][target_idx]
        };
        if self.pieces[c_idx][piece_index(Piece::Pawn)].0 & pawn_sources != 0 {
            return true;
        }

        if self.pieces[c_idx][piece_index(Piece::Knight)].0 & KNIGHT_ATTACKS[target_idx] != 0 {
            return true;
        }

        if self.pieces[c_idx][piece_index(Piece::King)].0 & KING_ATTACKS[target_idx] != 0 {
            return true;
        }

        let rook_like = self.pieces[c_idx][piece_index(Piece::Rook)].0
            | self.pieces[c_idx][piece_index(Piece::Queen)].0;
        let bishop_like = self.pieces[c_idx][piece_index(Piece::Bishop)].0
            | self.pieces[c_idx][piece_index(Piece::Queen)].0;

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
            self.is_square_attacked(king_sq, self.opponent_color(color))
        } else {
            false
        }
    }
}
