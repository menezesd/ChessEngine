use super::{bit_for_square, castle_bit, Board, Color, Piece, Square};

impl Board {
    pub(crate) fn has_castling_right(&self, color: Color, side: char) -> bool {
        self.castling_rights & castle_bit(color, side) != 0
    }

    pub(crate) fn set_piece(&mut self, sq: Square, color: Color, piece: Piece) {
        let bit = bit_for_square(sq).0;
        let c_idx = color.index();
        let p_idx = piece.index();
        self.pieces[c_idx][p_idx].0 |= bit;
        self.occupied[c_idx].0 |= bit;
        self.all_occupied.0 |= bit;
        self.mailbox[sq.index()] = Some((color, piece));
        if piece == Piece::King {
            self.king_square[c_idx] = sq;
        }
    }

    pub(crate) fn remove_piece(&mut self, sq: Square, color: Color, piece: Piece) {
        let bit = bit_for_square(sq).0;
        let c_idx = color.index();
        let p_idx = piece.index();
        self.pieces[c_idx][p_idx].0 &= !bit;
        self.occupied[c_idx].0 &= !bit;
        self.all_occupied.0 &= !bit;
        self.mailbox[sq.index()] = None;
    }

    /// O(1) lookup of piece at a square using mailbox representation.
    #[inline]
    pub(crate) fn piece_at(&self, sq: Square) -> Option<(Color, Piece)> {
        self.mailbox[sq.index()]
    }

    pub(crate) fn is_empty(&self, sq: Square) -> bool {
        self.all_occupied.0 & bit_for_square(sq).0 == 0
    }

    /// Get just the piece type on a square (without color).
    #[must_use]
    pub fn piece_on(&self, sq: Square) -> Option<Piece> {
        self.piece_at(sq).map(|(_, piece)| piece)
    }

    /// Get just the color of the piece on a square.
    #[must_use]
    pub fn color_on(&self, sq: Square) -> Option<Color> {
        self.piece_at(sq).map(|(color, _)| color)
    }

    /// Get the bitboard of pieces of a specific color and type.
    #[inline]
    #[must_use]
    pub fn pieces_of(&self, color: Color, piece: Piece) -> super::Bitboard {
        self.pieces[color.index()][piece.index()]
    }

    /// Get the bitboard of pieces of a specific type for the opponent of the given color.
    #[inline]
    #[must_use]
    pub fn opponent_pieces(&self, color: Color, piece: Piece) -> super::Bitboard {
        self.pieces[color.opponent().index()][piece.index()]
    }

    /// Get the bitboard of all pieces of a specific color.
    #[inline]
    #[must_use]
    pub fn occupied_by(&self, color: Color) -> super::Bitboard {
        self.occupied[color.index()]
    }
}
