mod kings;
mod knights;
mod pawns;
mod sliders;

use super::{pop_lsb, Board, Color, Move, MoveList, Piece, Square};

impl Board {
    fn generate_pseudo_moves(&self) -> MoveList {
        let mut moves = MoveList::new();
        let color = if self.white_to_move {
            Color::White
        } else {
            Color::Black
        };
        let c_idx = color.index();

        let mut pawns = self.pieces[c_idx][Piece::Pawn.index()];
        while pawns.0 != 0 {
            let from = Square::from_index(pop_lsb(&mut pawns));
            let pawn_moves = self.generate_pawn_moves(from);
            for m in pawn_moves.iter() {
                moves.push(*m);
            }
        }

        let mut knights = self.pieces[c_idx][Piece::Knight.index()];
        while knights.0 != 0 {
            let from = Square::from_index(pop_lsb(&mut knights));
            let knight_moves = self.generate_knight_moves(from);
            for m in knight_moves.iter() {
                moves.push(*m);
            }
        }

        let mut bishops = self.pieces[c_idx][Piece::Bishop.index()];
        while bishops.0 != 0 {
            let from = Square::from_index(pop_lsb(&mut bishops));
            let bishop_moves =
                self.generate_sliding_moves(from, &[(1, 1), (1, -1), (-1, 1), (-1, -1)]);
            for m in bishop_moves.iter() {
                moves.push(*m);
            }
        }

        let mut rooks = self.pieces[c_idx][Piece::Rook.index()];
        while rooks.0 != 0 {
            let from = Square::from_index(pop_lsb(&mut rooks));
            let rook_moves = self.generate_sliding_moves(from, &[(1, 0), (-1, 0), (0, 1), (0, -1)]);
            for m in rook_moves.iter() {
                moves.push(*m);
            }
        }

        let mut queens = self.pieces[c_idx][Piece::Queen.index()];
        while queens.0 != 0 {
            let from = Square::from_index(pop_lsb(&mut queens));
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

        let mut kings = self.pieces[c_idx][Piece::King.index()];
        while kings.0 != 0 {
            let from = Square::from_index(pop_lsb(&mut kings));
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

    /// Filter and collect capture moves from a piece's move list
    fn collect_captures(&self, piece_moves: &MoveList, dest: &mut MoveList) {
        for m in piece_moves.iter() {
            if m.captured_piece.is_some() {
                dest.push(*m);
            }
        }
    }

    pub(crate) fn generate_tactical_moves(&mut self) -> MoveList {
        let current_color = self.current_color();
        let mut pseudo_tactical_moves = MoveList::new();
        let c_idx = current_color.index();

        // Pawns have special tactical move generation (includes promotions)
        let mut pawns = self.pieces[c_idx][Piece::Pawn.index()];
        while pawns.0 != 0 {
            let from = Square::from_index(pop_lsb(&mut pawns));
            self.generate_pawn_tactical_moves(from, &mut pseudo_tactical_moves);
        }

        // For other pieces, filter captures from their normal moves
        for piece in [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen, Piece::King] {
            let mut pieces = self.pieces[c_idx][piece.index()];
            while pieces.0 != 0 {
                let from = Square::from_index(pop_lsb(&mut pieces));
                let piece_moves = self.generate_piece_moves(from, piece);
                self.collect_captures(&piece_moves, &mut pseudo_tactical_moves);
            }
        }

        // Filter for legality
        let mut legal_tactical_moves = MoveList::new();
        for m in pseudo_tactical_moves.iter() {
            let info = self.make_move(m);
            if !self.is_in_check(current_color) {
                legal_tactical_moves.push(*m);
            }
            self.unmake_move(m, info);
        }

        legal_tactical_moves
    }

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
