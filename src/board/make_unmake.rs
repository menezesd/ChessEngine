#![allow(clippy::trivially_copy_pass_by_ref)] // &Move is preferred for consistency

use super::{Board, Color, Move, NullMoveInfo, Piece, UnmakeInfo};
use crate::zobrist::ZOBRIST;

impl Board {
    pub(crate) fn make_move(&mut self, m: Move) -> UnmakeInfo {
        self.make_move_inner::<true>(m)
    }

    /// Search tracks its own line while retaining the actual game's history.
    pub(super) fn make_search_move(&mut self, m: Move) -> UnmakeInfo {
        self.make_move_inner::<false>(m)
    }

    fn make_move_inner<const UPDATE_HISTORY: bool>(&mut self, m: Move) -> UnmakeInfo {
        let previous_hash = self.hash;
        let mut current_hash = self.hash;

        // Save state for unmake
        let previous_en_passant_target = self.en_passant_target;
        let previous_castling_rights = self.castling_rights;
        let previous_halfmove_clock = self.halfmove_clock;
        let previous_fullmove_number = self.fullmove_number;
        let previous_eval = self.eval_state();

        let color = self.side_to_move();
        // Flip side to move in hash
        current_hash ^= ZOBRIST.black_to_move_key;

        // Remove old en passant from hash
        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= self.en_passant_hash_component(old_ep, color);
        }

        // Handle captures
        let (captured_piece_info, capture_hash_delta) = self.capture_piece_for_move(m);
        current_hash ^= capture_hash_delta;

        let (moving_color, moving_piece) = self.piece_at(m.from()).expect("make_move 'from' empty");
        debug_assert_eq!(moving_color, color);
        current_hash ^= self.apply_piece_move(m, color, moving_piece);

        // Handle double pawn push - set new en passant target
        current_hash ^= self.update_en_passant_target(m, color.opponent());

        // Update halfmove clock
        self.update_halfmove_clock(moving_piece, m.is_capture());
        if color == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }

        // Update castling rights
        current_hash ^= self.update_castling_rights(&m, moving_piece, color, captured_piece_info);

        self.white_to_move = !self.white_to_move;
        self.hash = current_hash;

        let previous_repetition_count =
            UPDATE_HISTORY.then(|| self.repetition_counts.record(current_hash));

        UnmakeInfo {
            captured_piece_info,
            previous_en_passant_target,
            previous_castling_rights,
            previous_hash,
            previous_halfmove_clock,
            previous_fullmove_number,
            previous_repetition_count,
            previous_eval,
        }
    }

    pub(crate) fn make_null_move(&mut self) -> NullMoveInfo {
        let previous_hash = self.hash;
        let previous_en_passant_target = self.en_passant_target;
        let previous_fullmove_number = self.fullmove_number;
        let mut current_hash = self.hash;
        let color = self.side_to_move();

        current_hash ^= ZOBRIST.black_to_move_key;
        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= self.en_passant_hash_component(old_ep, color);
        }
        self.en_passant_target = None;
        if color == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }
        self.white_to_move = !self.white_to_move;
        self.hash = current_hash;

        NullMoveInfo {
            previous_en_passant_target,
            previous_hash,
            fullmove_number: previous_fullmove_number,
        }
    }

    fn restore_castling_move(&mut self, m: Move, color: Color) {
        self.relocate_piece(m.to(), m.from(), color, Piece::King);

        let (rook_from, rook_to) = m.castling_rook_squares();
        self.relocate_piece(rook_to, rook_from, color, Piece::Rook);
    }

    fn restore_standard_move(&mut self, m: Move, color: Color, info: &UnmakeInfo) {
        let moved_piece_at_to = self
            .piece_at(m.to())
            .expect("Unmake move: 'to' square empty?");
        if m.promotion().is_some() {
            self.remove_piece(m.to(), moved_piece_at_to.0, moved_piece_at_to.1);
            self.set_piece(m.from(), color, Piece::Pawn);
        } else {
            self.relocate_piece(m.to(), m.from(), moved_piece_at_to.0, moved_piece_at_to.1);
        }

        if let Some((cap_col, cap_piece)) = info.captured_piece_info {
            self.set_piece(m.capture_square(), cap_col, cap_piece);
        }
    }

    pub(crate) fn unmake_move(&mut self, m: Move, info: UnmakeInfo) {
        if let Some(count) = info.previous_repetition_count {
            // The current hash still describes the child being removed.
            self.repetition_counts.set(self.hash, count);
        }

        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash;
        self.halfmove_clock = info.previous_halfmove_clock;
        self.fullmove_number = info.previous_fullmove_number;

        self.set_eval_state(info.previous_eval);

        let color = self.side_to_move();

        if m.is_castling() {
            self.restore_castling_move(m, color);
        } else {
            self.restore_standard_move(m, color, &info);
        }
    }

    pub(crate) fn unmake_null_move(&mut self, info: NullMoveInfo) {
        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.hash = info.previous_hash;
        self.fullmove_number = info.fullmove_number;
    }
}
