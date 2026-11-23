mod kings;
mod pawns;
mod sliders;

use self::sliders::SliderType;
use super::attack_tables::{
    bishop_attacks, diagonal_rays, pawn_attacks_from_bitboard, rook_attacks, straight_rays,
    KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS,
};
use super::{Bitboard, Board, Color, Move, MoveList, Piece, Square};

impl Board {
    fn generate_piece_moves(&self, from: Square, piece: Piece, moves: &mut MoveList) {
        match piece {
            Piece::Pawn => self.generate_pawn_moves(from, moves),
            Piece::King => self.generate_king_moves(from, moves),
            _ => self.generate_piece_steps(
                from,
                piece,
                !self.occupied_by(self.side_to_move()).0,
                moves,
            ),
        }
    }

    fn generate_piece_steps(&self, from: Square, piece: Piece, targets: u64, moves: &mut MoveList) {
        for to in Bitboard(self.non_pawn_attacks(from, piece) & targets).iter() {
            moves.push(self.create_simple_move(from, to));
        }
    }

    fn non_pawn_attacks(&self, from: Square, piece: Piece) -> u64 {
        match piece {
            Piece::Pawn => unreachable!("pawn attacks depend on color"),
            Piece::Knight => KNIGHT_ATTACKS[from.index()],
            Piece::Bishop => self.slider_targets(from, SliderType::Bishop),
            Piece::Rook => self.slider_targets(from, SliderType::Rook),
            Piece::Queen => self.slider_targets(from, SliderType::Queen),
            Piece::King => KING_ATTACKS[from.index()],
        }
    }

    /// Create a simple move (quiet or capture), the most common case.
    #[inline]
    fn create_simple_move(&self, from: Square, to: Square) -> Move {
        if self.piece_at(to).is_some() {
            Move::capture(from, to)
        } else {
            Move::quiet(from, to)
        }
    }

    /// Create a promotion move (with or without capture).
    #[inline]
    fn create_promotion_move(&self, from: Square, to: Square, piece: Piece) -> Move {
        if self.piece_at(to).is_some() {
            Move::new_promotion_capture(from, to, piece)
        } else {
            Move::new_promotion(from, to, piece)
        }
    }

    /// Create a castling move (kingside or queenside based on target file).
    #[inline]
    fn create_castling_move(from: Square, to: Square) -> Move {
        if to.file() == 6 {
            Move::castle_kingside(from, to)
        } else {
            Move::castle_queenside(from, to)
        }
    }

    #[must_use]
    pub fn generate_moves(&mut self) -> MoveList {
        let mut moves = MoveList::new();
        self.generate_moves_into(&mut moves);
        moves
    }

    pub(in crate::board) fn generate_moves_into(&self, moves: &mut MoveList) {
        self.generate_pseudo_moves_into(moves);
        self.retain_legal_moves(moves);
    }

    pub(in crate::board) fn generate_evasions_into(&self, moves: &mut MoveList) {
        let color = self.side_to_move();
        let targets = self.check_evasion_targets(color);
        if targets == u64::MAX {
            self.generate_moves_into(moves);
            return;
        }

        moves.clear();
        if targets != 0 || self.en_passant_target.is_some() {
            for from in self.pieces_of(color, Piece::Pawn).iter() {
                self.generate_pawn_moves_to(from, targets, moves);
            }
        }
        let open_targets = !self.occupied_by(color).0;
        let blocks_and_captures = targets & open_targets;
        if blocks_and_captures != 0 {
            for piece in [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
                for from in self.pieces_of(color, piece).iter() {
                    self.generate_piece_steps(from, piece, blocks_and_captures, moves);
                }
            }
        }
        for from in self.pieces_of(color, Piece::King).iter() {
            self.generate_piece_steps(from, Piece::King, open_targets, moves);
        }
        moves.retain(|mv| self.is_legal_pseudo_move(mv, color));
    }

    fn generate_pseudo_moves_into(&self, moves: &mut MoveList) {
        moves.clear();
        let color = self.side_to_move();
        for piece in Piece::ALL {
            for from in self.pieces_of(color, piece).iter() {
                self.generate_piece_moves(from, piece, moves);
            }
        }
    }

    fn check_evasion_targets(&self, color: Color) -> u64 {
        let king = self.find_king(color);
        let index = king.index();
        let attacker = color.opponent();
        let pieces = &self.pieces[attacker.index()];
        let queens = pieces[Piece::Queen.index()].0;
        let sliders = (rook_attacks(index, self.all_occupied.0)
            & (pieces[Piece::Rook.index()].0 | queens))
            | (bishop_attacks(index, self.all_occupied.0)
                & (pieces[Piece::Bishop.index()].0 | queens));
        let checkers = sliders
            | (PAWN_ATTACKS[color.index()][index] & pieces[Piece::Pawn.index()].0)
            | (KNIGHT_ATTACKS[index] & pieces[Piece::Knight.index()].0)
            | (KING_ATTACKS[index] & pieces[Piece::King.index()].0);

        if checkers == 0 {
            u64::MAX
        } else if !checkers.is_power_of_two() {
            // A king move must answer both checks. En passant has its own
            // legality check because it affects two squares at once.
            0
        } else if sliders == 0 {
            checkers
        } else {
            let checker = Square::from_index(checkers.trailing_zeros() as usize);
            let ranks = checker.rank() as i32 - king.rank() as i32;
            let files = checker.file() as i32 - king.file() as i32;
            let step = ranks.signum() * 8 + files.signum();
            let mut squares = checkers;
            for offset in 1..ranks.abs().max(files.abs()) {
                squares |= 1 << (king.index() as i32 + step * offset);
            }
            squares
        }
    }

    fn retain_legal_moves(&self, moves: &mut MoveList) {
        let color = self.side_to_move();
        let sensitive = if moves.len() > 3 {
            self.king_sensitive_squares(color)
        } else {
            u64::MAX
        };
        let kings = self.all_pieces_of_type(Piece::King).0;
        moves.retain(|mv| {
            if sensitive & (1 << mv.from().index()) != 0 || mv.is_en_passant() {
                self.is_legal_pseudo_move(mv, color)
            } else {
                kings & (1 << mv.to().index()) == 0
            }
        });
    }

    /// With the king out of check, only its own moves and moves off its
    /// first blockers can expose an attack. En passant is tested separately
    /// because it also removes a piece from a different square.
    fn king_sensitive_squares(&self, color: Color) -> u64 {
        let opponent = color.opponent();
        let king = self.find_king(color).index();
        if self.is_attacked_by_leaper(king, opponent, u64::MAX) {
            return u64::MAX;
        }
        let queens = self.pieces_of(opponent, Piece::Queen).0;
        let orthogonal = self.pieces_of(opponent, Piece::Rook).0 | queens;
        let diagonal = self.pieces_of(opponent, Piece::Bishop).0 | queens;
        let rook_ray = if orthogonal & straight_rays(king) != 0 {
            rook_attacks(king, self.all_occupied.0)
        } else {
            0
        };
        let bishop_ray = if diagonal & diagonal_rays(king) != 0 {
            bishop_attacks(king, self.all_occupied.0)
        } else {
            0
        };
        let attacks = (orthogonal & rook_ray) | (diagonal & bishop_ray);
        if attacks != 0 {
            u64::MAX
        } else {
            self.pieces_of(color, Piece::King).0 | rook_ray | bishop_ray
        }
    }

    /// Return whether the side to move has at least one legal move.
    ///
    /// This avoids materializing a complete legal move list for terminal-node
    /// checks, which is especially useful in quiescence search.
    pub(crate) fn has_legal_move(&self) -> bool {
        self.has_legal_move_except(super::EMPTY_MOVE)
    }

    /// Test for a legal move when the caller has already ruled out check.
    pub(in crate::board) fn has_legal_move_out_of_check(&self) -> bool {
        let color = self.side_to_move();
        let king = self.find_king(color).index();
        // An ordinary pawn push off the king's rays cannot expose a check.
        let sources =
            self.pieces_of(color, Piece::Pawn).0 & !(straight_rays(king) | diagonal_rays(king));
        self.ordinary_pawn_advances(sources, color) != 0 || self.has_legal_move()
    }

    /// Return whether the side to move has at least two legal moves.
    ///
    /// Search uses this before speculative node-level pruning so a forced
    /// reply is not skipped before the sole-reply extension can be applied.
    pub(crate) fn has_multiple_legal_moves(&self) -> bool {
        self.count_legal_moves_up_to(2, super::EMPTY_MOVE) == 2
    }

    /// Return whether the side to move has exactly one legal reply.
    pub(crate) fn has_single_legal_move(&self) -> bool {
        self.count_legal_moves_up_to(2, super::EMPTY_MOVE) == 1
    }

    /// Check for an alternative to a known move, stopping at the first one.
    pub(crate) fn has_legal_move_except(&self, excluded: Move) -> bool {
        self.count_legal_moves_up_to(1, excluded) != 0
    }

    #[inline]
    fn count_legal_moves_up_to(&self, limit: usize, excluded: Move) -> usize {
        let current_color = self.side_to_move();
        if self.has_pawn_advances(limit, excluded, current_color) {
            return limit;
        }
        let mut pseudo_moves = MoveList::new();
        let mut found = 0;
        for piece in Piece::ALL {
            for from in self.pieces_of(current_color, piece).iter() {
                pseudo_moves.clear();
                self.generate_piece_moves(from, piece, &mut pseudo_moves);
                for m in &pseudo_moves {
                    if *m != excluded && self.is_legal_pseudo_move(*m, current_color) {
                        found += 1;
                        if found == limit {
                            return found;
                        }
                    }
                }
            }
        }
        found
    }

    /// Look for ordinary pawn moves before allocating per-piece move lists.
    /// A bitboard finds the empty targets, then the normal pin/check test
    /// verifies each move that is needed as evidence.
    fn has_pawn_advances(&self, needed: usize, excluded: Move, color: Color) -> bool {
        let advances = self.ordinary_pawn_advances(self.pieces_of(color, Piece::Pawn).0, color);
        let mut found = 0;
        for to in Bitboard(advances).iter() {
            let from = Square::new(
                (to.rank() as isize - color.pawn_direction()) as usize,
                to.file(),
            );
            let mv = Move::quiet(from, to);
            if mv != excluded && self.is_legal_pseudo_move(mv, color) {
                found += 1;
                if found == needed {
                    return true;
                }
            }
        }
        false
    }

    fn ordinary_pawn_advances(&self, sources: u64, color: Color) -> u64 {
        let targets = match color {
            Color::White => (sources << 8) & !Bitboard::RANK_8.0,
            Color::Black => (sources >> 8) & !Bitboard::RANK_1.0,
        };
        targets & !self.all_occupied.0
    }

    pub(super) fn is_legal_pseudo_move(&self, m: Move, current_color: Color) -> bool {
        // Kings are checked, never captured.  Legal game flow cannot reach a
        // position where the side to move can take the opposing king, but an
        // externally supplied malformed FEN can.  Reject it before making
        // the move so the king cache and terminal logic stay intact.
        let from = Bitboard::from_square(m.from()).0;
        let to = Bitboard::from_square(m.to()).0;
        if self.all_pieces_of_type(Piece::King).0 & to != 0 {
            return false;
        }
        let king = if self.pieces_of(current_color, Piece::King).0 & from != 0 {
            m.to()
        } else {
            self.find_king(current_color)
        };

        let captured = if m.is_capture() {
            Bitboard::from_square(m.capture_square()).0
        } else {
            0
        };
        let mut occupancy = (self.all_occupied.0 & !from & !captured) | to;
        let opponent_color = current_color.opponent();
        if m.is_castling() {
            let (rook_from, transit) = m.castling_rook_squares();
            if self.is_square_attacked(m.from(), opponent_color)
                || self.is_square_attacked_with_occupancy(
                    transit,
                    opponent_color,
                    self.all_occupied.0 & !from,
                    0,
                )
            {
                return false;
            }
            occupancy &= !Bitboard::from_square(rook_from).0;
            occupancy |= Bitboard::from_square(transit).0;
        }

        !self.is_square_attacked_with_occupancy(king, opponent_color, occupancy, captured)
    }

    #[must_use]
    pub fn is_checkmate(&mut self) -> bool {
        let color = self.side_to_move();
        self.is_in_check(color) && !self.has_legal_move()
    }

    #[must_use]
    pub fn is_stalemate(&mut self) -> bool {
        let color = self.side_to_move();
        !self.is_in_check(color) && !self.has_legal_move()
    }

    /// Check if a move is legal without generating all moves.
    ///
    /// This is faster than `generate_moves().contains(&mv)` when you only need
    /// to validate a single move (e.g., from TT or user input).
    #[must_use]
    pub fn is_legal_move(&mut self, mv: Move) -> bool {
        self.legal_move_at(mv.from(), mv.to(), mv.promotion()) == Some(mv)
    }

    pub(in crate::board) fn legal_move_at(
        &self,
        from: Square,
        to: Square,
        promotion: Option<Piece>,
    ) -> Option<Move> {
        let current_color = self.side_to_move();
        let (piece_color, piece) = self.piece_at(from)?;
        if piece_color != current_color || self.occupied_by(current_color).contains(to) {
            return None;
        }
        let castling = piece == Piece::King
            && from.rank() == to.rank()
            && from.file().abs_diff(to.file()) == 2;
        let mv = if piece == Piece::Pawn || castling {
            let mut piece_moves = MoveList::new();
            self.generate_piece_moves(from, piece, &mut piece_moves);
            piece_moves
                .iter()
                .copied()
                .find(|candidate| candidate.to() == to && candidate.promotion() == promotion)?
        } else {
            if promotion.is_some() || self.non_pawn_attacks(from, piece) & (1 << to.index()) == 0 {
                return None;
            }
            self.create_simple_move(from, to)
        };
        self.is_legal_pseudo_move(mv, current_color).then_some(mv)
    }

    #[cfg(test)]
    pub(crate) fn generate_tactical_moves(&mut self) -> MoveList {
        let mut moves = MoveList::new();
        self.generate_tactical_moves_into(&mut moves);
        moves
    }

    pub(in crate::board) fn generate_tactical_moves_into(&self, moves: &mut MoveList) {
        moves.clear();
        let current_color = self.side_to_move();
        let enemy_pieces = self.occupied_by(current_color.opponent()).0;

        // Reverse pawn attacks find only sources that can capture or promote.
        let white = current_color == Color::White;
        let en_passant = self
            .en_passant_target
            .map_or(0, |square| 1 << square.index());
        let attackers = pawn_attacks_from_bitboard(enemy_pieces | en_passant, !white);
        let promotion_sources = Bitboard::rank_mask(if white { 6 } else { 1 }).0;
        let pawns = self.pieces_of(current_color, Piece::Pawn).0;
        for from in Bitboard(pawns & (attackers | promotion_sources)).iter() {
            self.generate_pawn_tactical_moves(from, moves);
        }

        // Mask attacks before building moves so quiet moves and castling
        // don't have to be generated and discarded in quiescence.
        for piece in [
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ] {
            for from in self.pieces_of(current_color, piece).iter() {
                let targets = self.non_pawn_attacks(from, piece) & enemy_pieces;
                for to in Bitboard(targets).iter() {
                    moves.push(Move::capture(from, to));
                }
            }
        }

        self.retain_legal_moves(moves);
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

#[cfg(test)]
mod tests {
    use super::{Board, MoveList};

    #[test]
    fn evasions_keep_en_passant_and_pawn_interpositions() {
        for (fen, evasion) in [
            ("7k/8/8/3pP3/4K3/8/8/8 w - d6 0 2", "e5d6"),
            ("r6K/3P4/8/8/8/8/8/7k w - - 0 1", "d7d8n"),
            ("k7/8/8/8/K6r/8/3P4/8 w - - 0 1", "d2d4"),
        ] {
            let mut board = Board::from_fen(fen);
            assert!(board.is_in_check(board.side_to_move()));
            let expected = board.generate_moves();
            let mv = board.parse_move(evasion).unwrap();
            let mut moves = MoveList::new();
            board.generate_evasions_into(&mut moves);
            assert_eq!(moves.as_slice(), expected.as_slice());
            assert!(moves.as_slice().contains(&mv));
        }
    }
}
