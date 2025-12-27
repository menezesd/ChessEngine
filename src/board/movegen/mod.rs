mod kings;
mod knights;
mod pawns;
mod sliders;

use self::sliders::SliderType;
use super::{Board, Color, Move, MoveList, Piece, Square};

impl Board {
    fn generate_pseudo_moves(&self) -> MoveList {
        let mut moves = MoveList::new();
        let color = if self.white_to_move {
            Color::White
        } else {
            Color::Black
        };
        let c_idx = color.index();

        for from_idx in self.pieces[c_idx][Piece::Pawn.index()].iter() {
            let from = from_idx;
            for m in &self.generate_pawn_moves(from) {
                moves.push(*m);
            }
        }

        for from_idx in self.pieces[c_idx][Piece::Knight.index()].iter() {
            let from = from_idx;
            for m in &self.generate_knight_moves(from) {
                moves.push(*m);
            }
        }

        for from_idx in self.pieces[c_idx][Piece::Bishop.index()].iter() {
            let from = from_idx;
            for m in &self.generate_slider_moves(from, SliderType::Bishop) {
                moves.push(*m);
            }
        }

        for from_idx in self.pieces[c_idx][Piece::Rook.index()].iter() {
            let from = from_idx;
            for m in &self.generate_slider_moves(from, SliderType::Rook) {
                moves.push(*m);
            }
        }

        for from_idx in self.pieces[c_idx][Piece::Queen.index()].iter() {
            let from = from_idx;
            for m in &self.generate_slider_moves(from, SliderType::Queen) {
                moves.push(*m);
            }
        }

        for from_idx in self.pieces[c_idx][Piece::King.index()].iter() {
            let from = from_idx;
            for m in &self.generate_king_moves(from) {
                moves.push(*m);
            }
        }
        moves
    }

    fn generate_piece_moves(&self, from: Square, piece: Piece) -> MoveList {
        match piece {
            Piece::Pawn => self.generate_pawn_moves(from),
            Piece::Knight => self.generate_knight_moves(from),
            Piece::Bishop => self.generate_slider_moves(from, SliderType::Bishop),
            Piece::Rook => self.generate_slider_moves(from, SliderType::Rook),
            Piece::Queen => self.generate_slider_moves(from, SliderType::Queen),
            Piece::King => self.generate_king_moves(from),
        }
    }

    /// Create a move, determining the correct type based on context
    fn create_move(
        &self,
        from: Square,
        to: Square,
        promotion: Option<Piece>,
        is_castling: bool,
        is_en_passant: bool,
        is_double_pawn_push: bool,
    ) -> Move {
        // Handle special move types
        if is_en_passant {
            return Move::en_passant(from, to);
        }

        if is_castling {
            return if to.file() == 6 {
                Move::castle_kingside(from, to)
            } else {
                Move::castle_queenside(from, to)
            };
        }

        if is_double_pawn_push {
            return Move::double_pawn_push(from, to);
        }

        // Check for promotion
        if let Some(promo_piece) = promotion {
            return if self.piece_at(to).is_some() {
                Move::new_promotion_capture(from, to, promo_piece)
            } else {
                Move::new_promotion(from, to, promo_piece)
            };
        }

        // Regular move - check for capture
        if self.piece_at(to).is_some() {
            Move::capture(from, to)
        } else {
            Move::quiet(from, to)
        }
    }

    #[must_use]
    pub fn generate_moves(&mut self) -> MoveList {
        let current_color = self.current_color();
        let opponent_color = current_color.opponent();
        let pseudo_moves = self.generate_pseudo_moves();
        let mut legal_moves = MoveList::new();

        for m in &pseudo_moves {
            if m.is_castling() {
                let king_start_sq = m.from();
                let from = m.from();
                let to = m.to();
                let king_mid_sq = Square::new(from.rank(), usize::midpoint(from.file(), to.file()));
                let king_end_sq = m.to();

                if self.is_square_attacked(king_start_sq, opponent_color)
                    || self.is_square_attacked(king_mid_sq, opponent_color)
                    || self.is_square_attacked(king_end_sq, opponent_color)
                {
                    continue;
                }
            }

            let info = self.make_move(*m);
            if !self.is_in_check(current_color) {
                legal_moves.push(*m);
            }
            self.unmake_move(*m, info);
        }
        legal_moves
    }

    #[must_use]
    pub fn is_checkmate(&mut self) -> bool {
        let color = self.current_color();
        self.is_in_check(color) && self.generate_moves().is_empty()
    }

    #[must_use]
    pub fn is_stalemate(&mut self) -> bool {
        let color = self.current_color();
        !self.is_in_check(color) && self.generate_moves().is_empty()
    }

    /// Check if a move is legal without generating all moves.
    ///
    /// This is faster than `generate_moves().contains(&mv)` when you only need
    /// to validate a single move (e.g., from TT or user input).
    #[must_use]
    pub fn is_legal_move(&mut self, mv: Move) -> bool {
        let from = mv.from();
        let current_color = self.current_color();

        // Check that there's a piece of the right color on the from square
        let Some((piece_color, piece)) = self.piece_at(from) else {
            return false;
        };
        if piece_color != current_color {
            return false;
        }

        // Generate pseudo-moves for just this piece
        let piece_moves = self.generate_piece_moves(from, piece);

        // Check if the move matches any pseudo-legal move
        if !piece_moves.iter().any(|m| *m == mv) {
            return false;
        }

        // For castling, check that king doesn't pass through check
        if mv.is_castling() {
            let opponent_color = current_color.opponent();
            let to = mv.to();
            let king_mid_sq = Square::new(from.rank(), usize::midpoint(from.file(), to.file()));

            if self.is_square_attacked(from, opponent_color)
                || self.is_square_attacked(king_mid_sq, opponent_color)
                || self.is_square_attacked(to, opponent_color)
            {
                return false;
            }
        }

        // Make the move and check if king is left in check
        let info = self.make_move(mv);
        let legal = !self.is_in_check(current_color);
        self.unmake_move(mv, info);

        legal
    }

    /// Filter and collect capture moves from a piece's move list
    fn collect_captures(piece_moves: &MoveList, dest: &mut MoveList) {
        for m in piece_moves {
            if m.is_capture() {
                dest.push(*m);
            }
        }
    }

    pub(crate) fn generate_tactical_moves(&mut self) -> MoveList {
        let current_color = self.current_color();
        let mut pseudo_tactical_moves = MoveList::new();
        let c_idx = current_color.index();

        // Pawns have special tactical move generation (includes promotions)
        for from_idx in self.pieces[c_idx][Piece::Pawn.index()].iter() {
            let from = from_idx;
            self.generate_pawn_tactical_moves(from, &mut pseudo_tactical_moves);
        }

        // For other pieces, filter captures from their normal moves
        for piece in [
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ] {
            for from_idx in self.pieces[c_idx][piece.index()].iter() {
                let from = from_idx;
                let piece_moves = self.generate_piece_moves(from, piece);
                Self::collect_captures(&piece_moves, &mut pseudo_tactical_moves);
            }
        }

        // Filter for legality
        let mut legal_tactical_moves = MoveList::new();
        for m in &pseudo_tactical_moves {
            let info = self.make_move(*m);
            if !self.is_in_check(current_color) {
                legal_tactical_moves.push(*m);
            }
            self.unmake_move(*m, info);
        }

        legal_tactical_moves
    }

    #[must_use]
    pub fn perft(&mut self, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }

        let moves = self.generate_moves();
        if depth == 1 {
            return moves.len() as u64;
        }

        let mut nodes = 0;
        for m in &moves {
            let info = self.make_move(*m);
            nodes += self.perft(depth - 1);
            self.unmake_move(*m, info);
        }

        nodes
    }
}
