#![allow(clippy::trivially_copy_pass_by_ref)] // &Move is preferred for consistency

use crate::zobrist::{
    color_to_zobrist_index, piece_to_zobrist_index, square_to_zobrist_index, ZOBRIST,
};

use super::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use super::eval_update::pst_square;
use super::pst::{MATERIAL_EG, MATERIAL_MG, PHASE_WEIGHTS, PST_EG, PST_MG};
use super::{castle_bit, Bitboard, Board, Color, Move, Piece, Square};

impl Board {
    fn revoke_castling_right(&mut self, color: Color, side: char, key_index: usize) -> u64 {
        if !self.has_castling_right(color, side) {
            return 0;
        }

        self.castling_rights &= !castle_bit(color, side);
        ZOBRIST.castling_keys[color_to_zobrist_index(color)][key_index]
    }

    /// Remove a captured piece, updating board state, hash, and incremental eval.
    /// Returns the hash XOR delta for the capture.
    #[inline]
    pub(super) fn remove_captured_piece(
        &mut self,
        capture_sq: Square,
        captured: (Color, Piece),
        opp_idx: usize,
    ) -> u64 {
        let (cap_col, cap_piece) = captured;
        let cap_sq_idx = capture_sq.index();
        let cap_p_idx = cap_piece.index();
        let cap_pst = pst_square(cap_sq_idx, cap_col == Color::White);

        self.remove_piece(capture_sq, cap_col, cap_piece);

        self.eval_mg[opp_idx] -= MATERIAL_MG[cap_p_idx] + PST_MG[cap_p_idx][cap_pst];
        self.eval_eg[opp_idx] -= MATERIAL_EG[cap_p_idx] + PST_EG[cap_p_idx][cap_pst];
        self.game_phase[opp_idx] -= PHASE_WEIGHTS[cap_p_idx];

        ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)][color_to_zobrist_index(cap_col)]
            [square_to_zobrist_index(capture_sq)]
    }

    /// Execute castling: move king (already removed), place king and rook.
    /// Returns the hash XOR delta for the rook movement.
    #[inline]
    pub(super) fn execute_castling(
        &mut self,
        m: &Move,
        color: Color,
        c_idx: usize,
        is_white: bool,
    ) -> u64 {
        let to_idx = m.to().index();
        let to_pst = pst_square(to_idx, is_white);

        self.set_piece(m.to(), color, Piece::King);

        let king_idx = Piece::King.index();
        self.eval_mg[c_idx] += MATERIAL_MG[king_idx] + PST_MG[king_idx][to_pst];
        self.eval_eg[c_idx] += MATERIAL_EG[king_idx] + PST_EG[king_idx][to_pst];
        self.game_phase[c_idx] += PHASE_WEIGHTS[king_idx];

        let (rook_from_f, rook_to_f) = if m.to().file() == 6 { (7, 5) } else { (0, 3) };
        let rook_from = Square::new(m.to().rank(), rook_from_f);
        let rook_to = Square::new(m.to().rank(), rook_to_f);
        let rook_from_idx = rook_from.index();
        let rook_to_idx = rook_to.index();

        let rook_info = self.piece_at(rook_from).expect("Castling without rook");
        self.remove_piece(rook_from, rook_info.0, rook_info.1);
        self.set_piece(rook_to, rook_info.0, rook_info.1);

        let rook_idx = Piece::Rook.index();
        let rook_from_pst = pst_square(rook_from_idx, is_white);
        let rook_to_pst = pst_square(rook_to_idx, is_white);
        self.eval_mg[c_idx] -= MATERIAL_MG[rook_idx] + PST_MG[rook_idx][rook_from_pst];
        self.eval_eg[c_idx] -= MATERIAL_EG[rook_idx] + PST_EG[rook_idx][rook_from_pst];
        self.eval_mg[c_idx] += MATERIAL_MG[rook_idx] + PST_MG[rook_idx][rook_to_pst];
        self.eval_eg[c_idx] += MATERIAL_EG[rook_idx] + PST_EG[rook_idx][rook_to_pst];

        ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)][color_to_zobrist_index(color)]
            [square_to_zobrist_index(rook_from)]
            ^ ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)][color_to_zobrist_index(color)]
                [square_to_zobrist_index(rook_to)]
    }

    /// Update castling rights based on a move.
    /// Returns the hash XOR delta for castling rights changes.
    #[inline]
    pub(super) fn update_castling_rights(
        &mut self,
        m: &Move,
        moving_piece: Piece,
        color: Color,
        captured: Option<(Color, Piece)>,
    ) -> u64 {
        let mut hash_delta: u64 = 0;

        if moving_piece == Piece::King {
            hash_delta ^= self.revoke_castling_right(color, 'K', 0);
            hash_delta ^= self.revoke_castling_right(color, 'Q', 1);
        } else if moving_piece == Piece::Rook {
            let start_rank = if color == Color::White { 0 } else { 7 };
            if m.from() == Square::new(start_rank, 0) {
                hash_delta ^= self.revoke_castling_right(color, 'Q', 1);
            } else if m.from() == Square::new(start_rank, 7) {
                hash_delta ^= self.revoke_castling_right(color, 'K', 0);
            }
        }

        if let Some((captured_color, Piece::Rook)) = captured {
            let start_rank = if captured_color == Color::White { 0 } else { 7 };
            if m.to() == Square::new(start_rank, 0) {
                hash_delta ^= self.revoke_castling_right(captured_color, 'Q', 1);
            } else if m.to() == Square::new(start_rank, 7) {
                hash_delta ^= self.revoke_castling_right(captured_color, 'K', 0);
            }
        }

        hash_delta
    }

    /// Remove captured piece for a move (including en passant) and return hash delta.
    pub(super) fn capture_piece_for_move(
        &mut self,
        m: Move,
        is_white: bool,
        opp_idx: usize,
    ) -> (Option<(Color, Piece)>, u64) {
        if m.is_en_passant() {
            let capture_sq = Self::en_passant_capture_square(m, is_white);
            if let Some(captured) = self.piece_at(capture_sq) {
                let delta = self.remove_captured_piece(capture_sq, captured, opp_idx);
                return (Some(captured), delta);
            }
            return (None, 0);
        }

        if m.is_castling() {
            return (None, 0);
        }

        if let Some(captured) = self.piece_at(m.to()) {
            let delta = self.remove_captured_piece(m.to(), captured, opp_idx);
            (Some(captured), delta)
        } else {
            (None, 0)
        }
    }

    /// Place the moving piece (and rook for castling), updating eval and returning hash delta.
    pub(super) fn place_moving_piece(
        &mut self,
        m: Move,
        color: Color,
        moving_piece: Piece,
        c_idx: usize,
        is_white: bool,
    ) -> u64 {
        if m.is_castling() {
            let king_hash = ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::King)]
                [color_to_zobrist_index(color)][square_to_zobrist_index(m.to())];
            return king_hash ^ self.execute_castling(&m, color, c_idx, is_white);
        }

        let piece_to_place = m.promotion().unwrap_or(moving_piece);
        self.set_piece(m.to(), color, piece_to_place);

        let placed_idx = piece_to_place.index();
        let to_idx = m.to().index();
        let to_pst = pst_square(to_idx, is_white);
        self.eval_mg[c_idx] += MATERIAL_MG[placed_idx] + PST_MG[placed_idx][to_pst];
        self.eval_eg[c_idx] += MATERIAL_EG[placed_idx] + PST_EG[placed_idx][to_pst];
        self.game_phase[c_idx] += PHASE_WEIGHTS[placed_idx];

        ZOBRIST.piece_keys[piece_to_zobrist_index(piece_to_place)][color_to_zobrist_index(color)]
            [square_to_zobrist_index(m.to())]
    }

    /// Return the en-passant hash component when the given side can capture it.
    ///
    /// Only a legal en-passant capture changes the set of legal moves, so an
    /// adjacent pawn pinned to its king must not distinguish positions for
    /// repetition detection.
    pub(super) fn en_passant_hash_component(&self, ep_square: Square, capturer: Color) -> u64 {
        let pawn_rank = match capturer {
            Color::White if ep_square.rank() > 0 => ep_square.rank() - 1,
            Color::Black if ep_square.rank() < 7 => ep_square.rank() + 1,
            Color::White | Color::Black => return 0,
        };
        let captured_square = Square::new(pawn_rank, ep_square.file());
        if !self.is_empty(ep_square)
            || self.piece_at(captured_square) != Some((capturer.opponent(), Piece::Pawn))
        {
            return 0;
        }

        let file = ep_square.file();
        let has_legal_capture = [file.checked_sub(1), file.checked_add(1)]
            .into_iter()
            .flatten()
            .filter(|candidate| *candidate < 8)
            .any(|candidate| {
                let from = Square::new(pawn_rank, candidate);
                self.piece_at(from) == Some((capturer, Piece::Pawn))
                    && self.en_passant_capture_is_legal(from, ep_square, captured_square, capturer)
            });

        if has_legal_capture {
            ZOBRIST.en_passant_keys[file]
        } else {
            0
        }
    }

    /// Whether the given en-passant capture leaves the capturer's king safe.
    ///
    /// This is used while constructing a position hash, so it cannot make and
    /// unmake a move. Recompute attacks on the king with just the three
    /// en-passant occupancy changes applied instead.
    fn en_passant_capture_is_legal(
        &self,
        from: Square,
        ep_square: Square,
        captured_square: Square,
        capturer: Color,
    ) -> bool {
        let opponent = capturer.opponent();
        let king_square = self.find_king(capturer);
        let king_index = king_square.index();
        let occupancy = (self.all_occupied.0
            & !Bitboard::from_square(from).0
            & !Bitboard::from_square(captured_square).0)
            | Bitboard::from_square(ep_square).0;

        let pawn_sources = if opponent == Color::White {
            PAWN_ATTACKS[Color::Black.index()][king_index]
        } else {
            PAWN_ATTACKS[Color::White.index()][king_index]
        };
        let opponent_pawns =
            self.pieces_of(opponent, Piece::Pawn).0 & !Bitboard::from_square(captured_square).0;
        if opponent_pawns & pawn_sources != 0 {
            return false;
        }
        if self.pieces_of(opponent, Piece::Knight).0 & KNIGHT_ATTACKS[king_index] != 0 {
            return false;
        }
        if self.pieces_of(opponent, Piece::King).0 & KING_ATTACKS[king_index] != 0 {
            return false;
        }

        let rook_like =
            self.pieces_of(opponent, Piece::Rook).0 | self.pieces_of(opponent, Piece::Queen).0;
        if slider_attacks(king_index, occupancy, false) & rook_like != 0 {
            return false;
        }
        let bishop_like =
            self.pieces_of(opponent, Piece::Bishop).0 | self.pieces_of(opponent, Piece::Queen).0;
        slider_attacks(king_index, occupancy, true) & bishop_like == 0
    }

    /// Update en passant target based on the move and return hash delta.
    pub(super) fn update_en_passant_target(&mut self, m: Move, capturer: Color) -> u64 {
        self.en_passant_target = None;
        if m.is_double_pawn_push() {
            let ep_row = usize::midpoint(m.from().rank(), m.to().rank());
            let ep_sq = Square::new(ep_row, m.from().file());
            self.en_passant_target = Some(ep_sq);
            return self.en_passant_hash_component(ep_sq, capturer);
        }
        0
    }

    /// Update halfmove clock after a move.
    pub(super) fn update_halfmove_clock(&mut self, moving_piece: Piece, is_capture: bool) {
        if moving_piece == Piece::Pawn || is_capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        }
    }

    /// Record repetition info and return the previous count.
    pub(super) fn record_repetition(&mut self, made_hash: u64) -> u32 {
        let previous_repetition_count = self.repetition_counts.get(made_hash);
        self.repetition_counts.increment(made_hash);
        previous_repetition_count
    }

    pub(super) fn en_passant_capture_square(m: Move, is_white: bool) -> Square {
        let capture_row = if is_white {
            m.to().rank() - 1
        } else {
            m.to().rank() + 1
        };
        Square::new(capture_row, m.to().file())
    }
}
