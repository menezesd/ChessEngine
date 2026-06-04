#![allow(clippy::trivially_copy_pass_by_ref)] // &Move is preferred for consistency

use crate::zobrist::{
    color_to_zobrist_index, piece_to_zobrist_index, square_to_zobrist_index, ZOBRIST,
};

use super::eval_update::pst_square;
use super::pst::{MATERIAL_EG, MATERIAL_MG, PHASE_WEIGHTS, PST_EG, PST_MG};
use super::{Board, Color, Move, NullMoveInfo, Piece, Square, UnmakeInfo};

impl Board {
    pub(crate) fn make_move(&mut self, m: Move) -> UnmakeInfo {
        let previous_hash = self.hash;
        let mut current_hash = self.hash;

        // Save state for unmake
        let previous_en_passant_target = self.en_passant_target;
        let previous_castling_rights = self.castling_rights;
        let previous_halfmove_clock = self.halfmove_clock;
        let previous_eval_mg = self.eval_mg;
        let previous_eval_eg = self.eval_eg;
        let previous_game_phase = self.game_phase;

        let color = self.side_to_move();
        let c_idx = color.index();
        let opp_idx = 1 - c_idx;
        let is_white = color == Color::White;

        // Flip side to move in hash
        current_hash ^= ZOBRIST.black_to_move_key;

        // Remove old en passant from hash
        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= ZOBRIST.en_passant_keys[old_ep.file()];
        }

        // Handle captures
        let (captured_piece_info, capture_hash_delta) =
            self.capture_piece_for_move(m, is_white, opp_idx);
        current_hash ^= capture_hash_delta;

        // Get moving piece info and remove from source square
        let moving_piece_info = self.piece_at(m.from()).expect("make_move 'from' empty");
        let (moving_color, moving_piece) = moving_piece_info;
        let piece_idx = moving_piece.index();
        let from_idx = m.from().index();
        // Remove moving piece from hash
        current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(moving_piece)]
            [color_to_zobrist_index(moving_color)][square_to_zobrist_index(m.from())];

        // Remove moving piece from board
        self.remove_piece(m.from(), moving_color, moving_piece);

        // Update eval: remove piece from 'from' square
        let from_pst = pst_square(from_idx, is_white);
        self.eval_mg[c_idx] -= MATERIAL_MG[piece_idx] + PST_MG[piece_idx][from_pst];
        self.eval_eg[c_idx] -= MATERIAL_EG[piece_idx] + PST_EG[piece_idx][from_pst];
        self.game_phase[c_idx] -= PHASE_WEIGHTS[piece_idx];

        current_hash ^= self.place_moving_piece(m, color, moving_piece, c_idx, is_white);

        // Handle double pawn push - set new en passant target
        current_hash ^= self.update_en_passant_target(m);

        // Update halfmove clock
        self.update_halfmove_clock(moving_piece, m.is_capture());

        // Update castling rights
        current_hash ^= self.update_castling_rights(&m, moving_piece, color, captured_piece_info);

        self.white_to_move = !self.white_to_move;
        self.hash = current_hash;

        let made_hash = current_hash;
        let previous_repetition_count = self.record_repetition(made_hash);

        UnmakeInfo {
            captured_piece_info,
            previous_en_passant_target,
            previous_castling_rights,
            previous_hash,
            previous_halfmove_clock,
            made_hash,
            previous_repetition_count,
            previous_eval_mg,
            previous_eval_eg,
            previous_game_phase,
        }
    }

    pub(crate) fn make_null_move(&mut self) -> NullMoveInfo {
        let previous_hash = self.hash;
        let previous_en_passant_target = self.en_passant_target;
        let mut current_hash = self.hash;

        current_hash ^= ZOBRIST.black_to_move_key;
        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= ZOBRIST.en_passant_keys[old_ep.file()];
        }
        self.en_passant_target = None;
        self.white_to_move = !self.white_to_move;
        self.hash = current_hash;

        NullMoveInfo {
            previous_en_passant_target,
            previous_hash,
        }
    }

    fn restore_castling_move(&mut self, m: Move, color: Color) {
        self.set_piece(m.from(), color, Piece::King);
        self.remove_piece(m.to(), color, Piece::King);

        let (rook_orig_f, rook_moved_f) = if m.to().file() == 6 { (7, 5) } else { (0, 3) };
        let rook_sq = Square::new(m.to().rank(), rook_moved_f);
        let rook_info = self
            .piece_at(rook_sq)
            .expect("Unmake castling: rook missing");
        self.remove_piece(rook_sq, rook_info.0, rook_info.1);
        self.set_piece(
            Square::new(m.to().rank(), rook_orig_f),
            rook_info.0,
            rook_info.1,
        );
    }

    fn restore_standard_move(&mut self, m: Move, color: Color, info: &UnmakeInfo) {
        let moved_piece_at_to = self
            .piece_at(m.to())
            .expect("Unmake move: 'to' square empty?");
        self.remove_piece(m.to(), moved_piece_at_to.0, moved_piece_at_to.1);
        let piece_on_from = if m.promotion().is_some() {
            (color, Piece::Pawn)
        } else {
            moved_piece_at_to
        };
        self.set_piece(m.from(), piece_on_from.0, piece_on_from.1);

        if m.is_en_passant() {
            if let Some((cap_col, cap_piece)) = info.captured_piece_info {
                let capture_sq = Self::en_passant_capture_square(m, color == Color::White);
                self.set_piece(capture_sq, cap_col, cap_piece);
            }
        } else if let Some((cap_col, cap_piece)) = info.captured_piece_info {
            self.set_piece(m.to(), cap_col, cap_piece);
        }
    }

    pub(crate) fn unmake_move(&mut self, m: Move, info: UnmakeInfo) {
        self.repetition_counts
            .set(info.made_hash, info.previous_repetition_count);

        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash;
        self.halfmove_clock = info.previous_halfmove_clock;

        // Restore incremental eval
        self.eval_mg = info.previous_eval_mg;
        self.eval_eg = info.previous_eval_eg;
        self.game_phase = info.previous_game_phase;

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
    }
}
