use super::super::{Board, Color, MoveList, Square, PROMOTION_PIECES};

impl Board {
    /// Add promotion moves for a pawn reaching the back rank
    fn add_promotions(&self, from: Square, to: Square, moves: &mut MoveList) {
        for promo in PROMOTION_PIECES {
            moves.push(self.create_move(from, to, Some(promo), false, false, false));
        }
    }

    /// Generate pawn capture moves (used by both regular and tactical move generation)
    fn generate_pawn_captures(
        &self,
        from: Square,
        color: Color,
        promotion_rank: usize,
        moves: &mut MoveList,
    ) {
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let r = from.rank() as isize;
        let f = from.file() as isize;
        let forward_r = r + dir;

        if !(0..8).contains(&forward_r) {
            return;
        }

        for df in [-1, 1] {
            let capture_f = f + df;
            if !(0..8).contains(&capture_f) {
                continue;
            }

            let target_sq = Square::new(forward_r as usize, capture_f as usize);

            if let Some((target_color, _)) = self.piece_at(target_sq) {
                if target_color != color {
                    if target_sq.rank() == promotion_rank {
                        self.add_promotions(from, target_sq, moves);
                    } else {
                        moves.push(self.create_move(from, target_sq, None, false, false, false));
                    }
                }
            } else if Some(target_sq) == self.en_passant_target {
                moves.push(self.create_move(from, target_sq, None, false, true, false));
            }
        }
    }

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

        let r = from.rank() as isize;
        let f = from.file() as isize;

        // Forward moves
        let forward_r = r + dir;
        if (0..8).contains(&forward_r) {
            let forward_sq = Square::new(forward_r as usize, f as usize);
            if self.is_empty(forward_sq) {
                if forward_sq.rank() == promotion_rank {
                    self.add_promotions(from, forward_sq, &mut moves);
                } else {
                    moves.push(self.create_move(from, forward_sq, None, false, false, false));
                    // Double push from starting rank
                    if r == start_rank as isize {
                        let double_forward_r = r + 2 * dir;
                        let double_forward_sq = Square::new(double_forward_r as usize, f as usize);
                        if self.is_empty(double_forward_sq) {
                            moves.push(self.create_move(
                                from,
                                double_forward_sq,
                                None,
                                false,
                                false,
                                true,
                            ));
                        }
                    }
                }
            }
        }

        // Captures (including en passant)
        self.generate_pawn_captures(from, color, promotion_rank, &mut moves);

        moves
    }

    pub(crate) fn generate_pawn_tactical_moves(&self, from: Square, moves: &mut MoveList) {
        let color = self.current_color();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        let r = from.rank() as isize;
        let f = from.file() as isize;
        let forward_r = r + dir;

        // Forward promotion (non-capture)
        if (0..8).contains(&forward_r) {
            let forward_sq = Square::new(forward_r as usize, f as usize);
            if forward_sq.rank() == promotion_rank && self.is_empty(forward_sq) {
                self.add_promotions(from, forward_sq, moves);
            }
        }

        // Captures (including en passant and capture-promotions)
        self.generate_pawn_captures(from, color, promotion_rank, moves);
    }
}
