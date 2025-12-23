use super::attack_tables::{
    slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS,
};
use super::{
    color_index, piece_index, pop_lsb, square_from_index, square_index, Bitboard, Board, Color,
    Move, MoveList, Piece, Square,
};

impl Board {
    pub(crate) fn mobility_counts(&self) -> (i32, i32) {
        let mut white = 0;
        let mut black = 0;

        let pieces = [
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
        ];

        for &color in &[Color::White, Color::Black] {
            let c_idx = color_index(color);
            let mut count = 0;
            for &piece in &pieces {
                let mut bb = self.pieces[c_idx][piece_index(piece)];
                while bb.0 != 0 {
                    let from = square_from_index(pop_lsb(&mut bb));
                    let moves = self.generate_piece_moves(from, piece);
                    count += moves.len() as i32;
                }
            }
            if color == Color::White {
                white = count;
            } else {
                black = count;
            }
        }

        (white, black)
    }

    fn generate_pseudo_moves(&self) -> MoveList {
        let mut moves = MoveList::new();
        let color = if self.white_to_move {
            Color::White
        } else {
            Color::Black
        };
        let c_idx = color_index(color);

        let mut pawns = self.pieces[c_idx][piece_index(Piece::Pawn)];
        while pawns.0 != 0 {
            let from = square_from_index(pop_lsb(&mut pawns));
            let pawn_moves = self.generate_pawn_moves(from);
            for m in pawn_moves.iter() {
                moves.push(*m);
            }
        }

        let mut knights = self.pieces[c_idx][piece_index(Piece::Knight)];
        while knights.0 != 0 {
            let from = square_from_index(pop_lsb(&mut knights));
            let knight_moves = self.generate_knight_moves(from);
            for m in knight_moves.iter() {
                moves.push(*m);
            }
        }

        let mut bishops = self.pieces[c_idx][piece_index(Piece::Bishop)];
        while bishops.0 != 0 {
            let from = square_from_index(pop_lsb(&mut bishops));
            let bishop_moves = self.generate_sliding_moves(
                from,
                &[(1, 1), (1, -1), (-1, 1), (-1, -1)],
            );
            for m in bishop_moves.iter() {
                moves.push(*m);
            }
        }

        let mut rooks = self.pieces[c_idx][piece_index(Piece::Rook)];
        while rooks.0 != 0 {
            let from = square_from_index(pop_lsb(&mut rooks));
            let rook_moves = self.generate_sliding_moves(
                from,
                &[(1, 0), (-1, 0), (0, 1), (0, -1)],
            );
            for m in rook_moves.iter() {
                moves.push(*m);
            }
        }

        let mut queens = self.pieces[c_idx][piece_index(Piece::Queen)];
        while queens.0 != 0 {
            let from = square_from_index(pop_lsb(&mut queens));
            let queen_moves = self.generate_sliding_moves(
                from,
                &[
                    (1, 0),
                    (-1, 0),
                    (0, 1),
                    (0, -1),
                    (1, 1),
                    (1, -1),
                    (-1, 1),
                    (-1, -1),
                ],
            );
            for m in queen_moves.iter() {
                moves.push(*m);
            }
        }

        let mut kings = self.pieces[c_idx][piece_index(Piece::King)];
        while kings.0 != 0 {
            let from = square_from_index(pop_lsb(&mut kings));
            let king_moves = self.generate_king_moves(from);
            for m in king_moves.iter() {
                moves.push(*m);
            }
        }
        moves
    }

    fn generate_piece_moves(&self, from: Square, piece: Piece) -> MoveList {
        match piece {
            Piece::Pawn => self.generate_pawn_moves(from),
            Piece::Knight => self.generate_knight_moves(from),
            Piece::Bishop => {
                self.generate_sliding_moves(from, &[(1, 1), (1, -1), (-1, 1), (-1, -1)])
            }
            Piece::Rook => self.generate_sliding_moves(from, &[(1, 0), (-1, 0), (0, 1), (0, -1)]),
            Piece::Queen => self.generate_sliding_moves(
                from,
                &[
                    (1, 0),
                    (-1, 0),
                    (0, 1),
                    (0, -1),
                    (1, 1),
                    (1, -1),
                    (-1, 1),
                    (-1, -1),
                ],
            ),
            Piece::King => self.generate_king_moves(from),
        }
    }

    fn create_move(
        &self,
        from: Square,
        to: Square,
        promotion: Option<Piece>,
        is_castling: bool,
        is_en_passant: bool,
    ) -> Move {
        let captured_piece = if is_en_passant {
            Some(Piece::Pawn)
        } else if !is_castling {
            self.piece_at(to).map(|(_, p)| p)
        } else {
            None
        };

        Move {
            from,
            to,
            promotion,
            is_castling,
            is_en_passant,
            captured_piece,
        }
    }

    fn generate_pawn_moves(&self, from: Square) -> MoveList {
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
        if forward_r >= 0 && forward_r < 8 {
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

        if forward_r >= 0 && forward_r < 8 {
            for df in [-1, 1] {
                let capture_f = f + df;
                if capture_f >= 0 && capture_f < 8 {
                    let target_sq = Square(forward_r as usize, capture_f as usize);
                    if let Some((target_color, _)) = self.piece_at(target_sq) {
                        if target_color != color {
                            if target_sq.0 == promotion_rank {
                                for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
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

    fn generate_knight_moves(&self, from: Square) -> MoveList {
        let mut moves = MoveList::new();
        let color = self.current_color();
        let from_idx = square_index(from).as_usize();
        let own_occ = self.occupied[color_index(color)].0;
        let mut targets = Bitboard(KNIGHT_ATTACKS[from_idx] & !own_occ);

        while targets.0 != 0 {
            let to_idx = pop_lsb(&mut targets);
            let to_sq = square_from_index(to_idx);
            moves.push(self.create_move(from, to_sq, None, false, false));
        }
        moves
    }

    fn generate_sliding_moves(&self, from: Square, directions: &[(isize, isize)]) -> MoveList {
        let mut moves = MoveList::new();
        let color = self.current_color();
        let own_occ = self.occupied[color_index(color)].0;
        let from_idx = square_index(from).as_usize();

        let targets = if directions.len() == 4 && directions[0].0 != 0 && directions[0].1 != 0 {
            slider_attacks(from_idx, self.all_occupied.0, true)
        } else if directions.len() == 4 {
            slider_attacks(from_idx, self.all_occupied.0, false)
        } else {
            slider_attacks(from_idx, self.all_occupied.0, false)
                | slider_attacks(from_idx, self.all_occupied.0, true)
        } & !own_occ;

        let mut targets = Bitboard(targets);
        while targets.0 != 0 {
            let to_idx = pop_lsb(&mut targets);
            let to_sq = square_from_index(to_idx);
            moves.push(self.create_move(from, to_sq, None, false, false));
        }
        moves
    }

    fn generate_king_moves(&self, from: Square) -> MoveList {
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

    pub fn generate_moves(&mut self) -> MoveList {
        let current_color = self.current_color();
        let opponent_color = self.opponent_color(current_color);
        let pseudo_moves = self.generate_pseudo_moves();
        let mut legal_moves = MoveList::new();

        for m in pseudo_moves.iter() {
            if m.is_castling {
                let king_start_sq = m.from;
                let king_mid_sq = Square(m.from.0, (m.from.1 + m.to.1) / 2);
                let king_end_sq = m.to;

                if self.is_square_attacked(king_start_sq, opponent_color)
                    || self.is_square_attacked(king_mid_sq, opponent_color)
                    || self.is_square_attacked(king_end_sq, opponent_color)
                {
                    continue;
                }
            }

            let info = self.make_move(m);
            if !self.is_in_check(current_color) {
                legal_moves.push(*m);
            }
            self.unmake_move(m, info);
        }
        legal_moves
    }

    pub fn is_checkmate(&mut self) -> bool {
        let color = self.current_color();
        self.is_in_check(color) && self.generate_moves().is_empty()
    }

    pub fn is_stalemate(&mut self) -> bool {
        let color = self.current_color();
        !self.is_in_check(color) && self.generate_moves().is_empty()
    }

    pub(crate) fn generate_tactical_moves(&mut self) -> MoveList {
        let current_color = self.current_color();

        let mut pseudo_tactical_moves = MoveList::new();
        let c_idx = color_index(current_color);

        let mut pawns = self.pieces[c_idx][piece_index(Piece::Pawn)];
        while pawns.0 != 0 {
            let from = square_from_index(pop_lsb(&mut pawns));
            self.generate_pawn_tactical_moves(from, &mut pseudo_tactical_moves);
        }

        let mut knights = self.pieces[c_idx][piece_index(Piece::Knight)];
        while knights.0 != 0 {
            let from = square_from_index(pop_lsb(&mut knights));
            let piece_moves = self.generate_piece_moves(from, Piece::Knight);
            for m in piece_moves.iter() {
                if m.captured_piece.is_some() || m.is_en_passant {
                    pseudo_tactical_moves.push(*m);
                }
            }
        }

        let mut bishops = self.pieces[c_idx][piece_index(Piece::Bishop)];
        while bishops.0 != 0 {
            let from = square_from_index(pop_lsb(&mut bishops));
            let piece_moves = self.generate_piece_moves(from, Piece::Bishop);
            for m in piece_moves.iter() {
                if m.captured_piece.is_some() || m.is_en_passant {
                    pseudo_tactical_moves.push(*m);
                }
            }
        }

        let mut rooks = self.pieces[c_idx][piece_index(Piece::Rook)];
        while rooks.0 != 0 {
            let from = square_from_index(pop_lsb(&mut rooks));
            let piece_moves = self.generate_piece_moves(from, Piece::Rook);
            for m in piece_moves.iter() {
                if m.captured_piece.is_some() || m.is_en_passant {
                    pseudo_tactical_moves.push(*m);
                }
            }
        }

        let mut queens = self.pieces[c_idx][piece_index(Piece::Queen)];
        while queens.0 != 0 {
            let from = square_from_index(pop_lsb(&mut queens));
            let piece_moves = self.generate_piece_moves(from, Piece::Queen);
            for m in piece_moves.iter() {
                if m.captured_piece.is_some() || m.is_en_passant {
                    pseudo_tactical_moves.push(*m);
                }
            }
        }

        let mut kings = self.pieces[c_idx][piece_index(Piece::King)];
        while kings.0 != 0 {
            let from = square_from_index(pop_lsb(&mut kings));
            let piece_moves = self.generate_piece_moves(from, Piece::King);
            for m in piece_moves.iter() {
                if m.captured_piece.is_some() || m.is_en_passant {
                    pseudo_tactical_moves.push(*m);
                }
            }
        }

        let mut legal_tactical_moves = MoveList::new();
        for m in pseudo_tactical_moves.iter() {
            if m.is_castling {
                continue;
            }

            let info = self.make_move(m);
            if !self.is_in_check(current_color) {
                legal_tactical_moves.push(*m);
            }
            self.unmake_move(m, info);
        }

        legal_tactical_moves
    }

    pub(crate) fn generate_checking_moves(&mut self) -> MoveList {
        let current_color = self.current_color();
        let pseudo_moves = self.generate_pseudo_moves();
        let mut checking_moves = MoveList::new();

        for m in pseudo_moves.iter() {
            if m.is_castling {
                continue;
            }
            let info = self.make_move(m);
            let gives_check = self.is_in_check(self.opponent_color(current_color));
            if gives_check {
                checking_moves.push(*m);
            }
            self.unmake_move(m, info);
        }

        checking_moves
    }

    fn generate_pawn_tactical_moves(&self, from: Square, moves: &mut MoveList) {
        let color = self.current_color();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        let r = from.0 as isize;
        let f = from.1 as isize;

        let forward_r = r + dir;

        if forward_r >= 0 && forward_r < 8 {
            let forward_sq = Square(forward_r as usize, f as usize);
            if forward_sq.0 == promotion_rank && self.is_empty(forward_sq) {
                for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                    moves.push(self.create_move(from, forward_sq, Some(promo), false, false));
                }
            }
        }

        if forward_r >= 0 && forward_r < 8 {
            for df in [-1, 1] {
                let capture_f = f + df;
                if capture_f >= 0 && capture_f < 8 {
                    let target_sq = Square(forward_r as usize, capture_f as usize);

                    if let Some((target_color, _)) = self.piece_at(target_sq) {
                        if target_color != color {
                            if target_sq.0 == promotion_rank {
                                for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
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

    #[allow(dead_code)]
    pub fn perft(&mut self, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }

        let moves = self.generate_moves();
        if depth == 1 {
            return moves.len() as u64;
        }

        let mut nodes = 0;
        for m in moves.iter() {
            let info = self.make_move(m);
            nodes += self.perft(depth - 1);
            self.unmake_move(m, info);
        }

        nodes
    }
}
