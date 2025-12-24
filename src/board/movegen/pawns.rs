use super::super::{Board, Color, MoveList, Piece, Square};

impl Board {
    pub(crate) fn generate_pawn_moves(&self, from: Square) -> MoveList {
        let color = if self.white_to_move {
            Color::White
        } else {
            Color::Black
        };
        let mut moves = MoveList::new();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let start_rank = if color == Color::White { 1 } else { 6 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        let r = from.0 as isize;
        let f = from.1 as isize;

        let forward_r = r + dir;
        if (0..8).contains(&forward_r) {
            let forward_sq = Square(forward_r as usize, f as usize);
            if self.is_empty(forward_sq) {
                if forward_sq.0 == promotion_rank {
                    for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                        moves.push(self.create_move(from, forward_sq, Some(promo), false, false));
                    }
                } else {
                    moves.push(self.create_move(from, forward_sq, None, false, false));
                    if r == start_rank as isize {
                        let double_forward_r = r + 2 * dir;
                        let double_forward_sq = Square(double_forward_r as usize, f as usize);
                        if self.is_empty(double_forward_sq) {
                            moves.push(self.create_move(
                                from,
                                double_forward_sq,
                                None,
                                false,
                                false,
                            ));
                        }
                    }
                }
            }
        }

        if (0..8).contains(&forward_r) {
            for df in [-1, 1] {
                let capture_f = f + df;
                if (0..8).contains(&capture_f) {
                    let target_sq = Square(forward_r as usize, capture_f as usize);
                    if let Some((target_color, _)) = self.piece_at(target_sq) {
                        if target_color != color {
                            if target_sq.0 == promotion_rank {
                                for promo in
                                    [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
                                {
                                    moves.push(self.create_move(
                                        from,
                                        target_sq,
                                        Some(promo),
                                        false,
                                        false,
                                    ));
                                }
                            } else {
                                moves.push(self.create_move(from, target_sq, None, false, false));
                            }
                        }
                    } else if Some(target_sq) == self.en_passant_target {
                        moves.push(self.create_move(from, target_sq, None, false, true));
                    }
                }
            }
        }

        moves
    }

    pub(crate) fn generate_pawn_tactical_moves(&self, from: Square, moves: &mut MoveList) {
        let color = self.current_color();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        let r = from.0 as isize;
        let f = from.1 as isize;

        let forward_r = r + dir;

        if (0..8).contains(&forward_r) {
            let forward_sq = Square(forward_r as usize, f as usize);
            if forward_sq.0 == promotion_rank && self.is_empty(forward_sq) {
                for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                    moves.push(self.create_move(from, forward_sq, Some(promo), false, false));
                }
            }
        }

        if (0..8).contains(&forward_r) {
            for df in [-1, 1] {
                let capture_f = f + df;
                if (0..8).contains(&capture_f) {
                    let target_sq = Square(forward_r as usize, capture_f as usize);

                    if let Some((target_color, _)) = self.piece_at(target_sq) {
                        if target_color != color {
                            if target_sq.0 == promotion_rank {
                                for promo in
                                    [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
                                {
                                    moves.push(self.create_move(
                                        from,
                                        target_sq,
                                        Some(promo),
                                        false,
                                        false,
                                    ));
                                }
                            } else {
                                moves.push(self.create_move(from, target_sq, None, false, false));
                            }
                        }
                    } else if Some(target_sq) == self.en_passant_target {
                        moves.push(self.create_move(from, target_sq, None, false, true));
                    }
                }
            }
        }
    }
}
