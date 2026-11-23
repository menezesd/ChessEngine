#![allow(clippy::trivially_copy_pass_by_ref)] // &Move is preferred for consistency

use crate::zobrist::ZOBRIST;

use super::attack_tables::PAWN_ATTACKS;
use super::{
    Bitboard, Board, Color, Move, Piece, Square, CASTLE_BLACK_K, CASTLE_BLACK_Q, CASTLE_WHITE_K,
    CASTLE_WHITE_Q,
};

const COLOR_CASTLING_RIGHTS: [u8; 2] = [
    CASTLE_WHITE_K | CASTLE_WHITE_Q,
    CASTLE_BLACK_K | CASTLE_BLACK_Q,
];
const ROOK_HOME_CASTLING_RIGHTS: [u8; 64] = {
    let mut rights = [0; 64];
    rights[Square::new(0, 0).index()] = CASTLE_WHITE_Q;
    rights[Square::new(0, 7).index()] = CASTLE_WHITE_K;
    rights[Square::new(7, 0).index()] = CASTLE_BLACK_Q;
    rights[Square::new(7, 7).index()] = CASTLE_BLACK_K;
    rights
};

impl Board {
    /// Remove a captured piece, updating board state, hash, and incremental eval.
    /// Returns the hash XOR delta for the capture.
    #[inline]
    fn remove_captured_piece(&mut self, capture_sq: Square, captured: (Color, Piece)) -> u64 {
        let (cap_col, cap_piece) = captured;
        let color_idx = cap_col.index();
        let cap_sq_idx = capture_sq.index();
        let cap_p_idx = cap_piece.index();
        self.remove_piece(capture_sq, cap_col, cap_piece);
        self.eval
            .remove_piece(color_idx, cap_piece, cap_sq_idx, cap_col == Color::White);
        ZOBRIST.piece_keys[cap_p_idx][color_idx][cap_sq_idx]
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
        let rights = self.castling_rights;
        if rights == 0 {
            return 0;
        }
        let own = COLOR_CASTLING_RIGHTS[color.index()];
        let mut removed = match moving_piece {
            Piece::King => own,
            Piece::Rook => own & ROOK_HOME_CASTLING_RIGHTS[m.from().index()],
            _ => 0,
        };
        if let Some((captured_color, Piece::Rook)) = captured {
            removed |= COLOR_CASTLING_RIGHTS[captured_color.index()]
                & ROOK_HOME_CASTLING_RIGHTS[m.to().index()];
        }
        removed &= rights;
        if removed == 0 {
            return 0;
        }
        self.castling_rights ^= removed;
        ZOBRIST.castling_keys[usize::from(removed)]
    }

    /// Remove captured piece for a move (including en passant) and return hash delta.
    pub(super) fn capture_piece_for_move(&mut self, m: Move) -> (Option<(Color, Piece)>, u64) {
        if !m.is_capture() {
            return (None, 0);
        }
        let capture_square = m.capture_square();
        if let Some(captured) = self.piece_at(capture_square) {
            let delta = self.remove_captured_piece(capture_square, captured);
            (Some(captured), delta)
        } else {
            (None, 0)
        }
    }

    /// Apply a move to the piece lists and evaluation and return its hash delta.
    pub(super) fn apply_piece_move(&mut self, m: Move, color: Color, piece: Piece) -> u64 {
        let mut delta = self.relocate_and_score(m.from(), m.to(), color, piece, m.promotion());
        if m.is_castling() {
            let (rook_from, rook_to) = m.castling_rook_squares();
            delta ^= self.relocate_and_score(rook_from, rook_to, color, Piece::Rook, None);
        }
        delta
    }

    fn relocate_and_score(
        &mut self,
        from: Square,
        to: Square,
        color: Color,
        piece: Piece,
        promotion: Option<Piece>,
    ) -> u64 {
        let c = color.index();
        let white = color == Color::White;
        if let Some(promoted) = promotion {
            self.remove_piece(from, color, piece);
            self.set_piece(to, color, promoted);
            self.eval.remove_piece(c, piece, from.index(), white);
            self.eval.add_piece(c, promoted, to.index(), white);
        } else {
            self.relocate_piece(from, to, color, piece);
            self.eval
                .move_piece(c, piece, from.index(), to.index(), white);
        }
        ZOBRIST.piece_keys[piece.index()][c][from.index()]
            ^ ZOBRIST.piece_keys[promotion.unwrap_or(piece).index()][c][to.index()]
    }

    /// Return the en-passant hash component when the given side can capture it.
    ///
    /// Only a legal en-passant capture changes the set of legal moves, so an
    /// adjacent pawn pinned to its king must not distinguish positions for
    /// repetition detection.
    pub(super) fn en_passant_hash_component(&self, ep_square: Square, capturer: Color) -> u64 {
        let opponent = capturer.opponent();
        let Some(captured_square) = ep_square.forward(opponent == Color::White) else {
            return 0;
        };
        if !self.is_empty(ep_square)
            || self.piece_at(captured_square) != Some((opponent, Piece::Pawn))
        {
            return 0;
        }

        let sources = self.pieces_of(capturer, Piece::Pawn).0
            & PAWN_ATTACKS[opponent.index()][ep_square.index()];
        if Bitboard(sources)
            .iter()
            .any(|from| self.is_legal_pseudo_move(Move::en_passant(from, ep_square), capturer))
        {
            ZOBRIST.en_passant_keys[ep_square.file()]
        } else {
            0
        }
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
}
