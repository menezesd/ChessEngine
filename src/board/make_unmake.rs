#![allow(clippy::trivially_copy_pass_by_ref)] // &Move is preferred for consistency

use crate::zobrist::{
    color_to_zobrist_index, piece_to_zobrist_index, square_to_zobrist_index, ZOBRIST,
};

use super::eval_update::pst_square;
use super::pst::{MATERIAL_EG, MATERIAL_MG, PHASE_WEIGHTS, PST_EG, PST_MG};
use super::{
    bit_for_square, castle_bit, Board, Color, Move, NullMoveInfo, Piece, Square, UnmakeInfo,
};

impl Board {
    pub(crate) fn current_color(&self) -> Color {
        if self.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    pub(crate) fn has_castling_right(&self, color: Color, side: char) -> bool {
        self.castling_rights & castle_bit(color, side) != 0
    }

    pub(crate) fn set_piece(&mut self, sq: Square, color: Color, piece: Piece) {
        let bit = bit_for_square(sq).0;
        let c_idx = color.index();
        let p_idx = piece.index();
        self.pieces[c_idx][p_idx].0 |= bit;
        self.occupied[c_idx].0 |= bit;
        self.all_occupied.0 |= bit;
    }

    pub(crate) fn remove_piece(&mut self, sq: Square, color: Color, piece: Piece) {
        let bit = bit_for_square(sq).0;
        let c_idx = color.index();
        let p_idx = piece.index();
        self.pieces[c_idx][p_idx].0 &= !bit;
        self.occupied[c_idx].0 &= !bit;
        self.all_occupied.0 &= !bit;
    }

    pub(crate) fn piece_at(&self, sq: Square) -> Option<(Color, Piece)> {
        let bit = bit_for_square(sq).0;
        if self.all_occupied.0 & bit == 0 {
            return None;
        }

        let color = if self.occupied[0].0 & bit != 0 {
            Color::White
        } else {
            Color::Black
        };
        let c_idx = color.index();
        for p_idx in 0..6 {
            if self.pieces[c_idx][p_idx].0 & bit != 0 {
                let piece = match p_idx {
                    0 => Piece::Pawn,
                    1 => Piece::Knight,
                    2 => Piece::Bishop,
                    3 => Piece::Rook,
                    4 => Piece::Queen,
                    5 => Piece::King,
                    _ => unreachable!(),
                };
                return Some((color, piece));
            }
        }

        None
    }

    pub(crate) fn is_empty(&self, sq: Square) -> bool {
        self.all_occupied.0 & bit_for_square(sq).0 == 0
    }

    /// Get just the piece type on a square (without color)
    #[must_use]
    pub fn piece_on(&self, sq: Square) -> Option<Piece> {
        self.piece_at(sq).map(|(_, piece)| piece)
    }

    /// Get just the color of the piece on a square
    #[must_use]
    pub fn color_on(&self, sq: Square) -> Option<Color> {
        self.piece_at(sq).map(|(color, _)| color)
    }

    pub(crate) fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;

        for r in 0..8 {
            for f in 0..8 {
                let sq = Square::new(r, f);
                if let Some((color, piece)) = self.piece_at(sq) {
                    let sq_idx = square_to_zobrist_index(sq);
                    let p_idx = piece_to_zobrist_index(piece);
                    let c_idx = color_to_zobrist_index(color);
                    hash ^= ZOBRIST.piece_keys[p_idx][c_idx][sq_idx];
                }
            }
        }

        if !self.white_to_move {
            hash ^= ZOBRIST.black_to_move_key;
        }

        if self.castling_rights & super::CASTLE_WHITE_K != 0 {
            hash ^= ZOBRIST.castling_keys[0][0];
        }
        if self.castling_rights & super::CASTLE_WHITE_Q != 0 {
            hash ^= ZOBRIST.castling_keys[0][1];
        }
        if self.castling_rights & super::CASTLE_BLACK_K != 0 {
            hash ^= ZOBRIST.castling_keys[1][0];
        }
        if self.castling_rights & super::CASTLE_BLACK_Q != 0 {
            hash ^= ZOBRIST.castling_keys[1][1];
        }

        if let Some(ep_square) = self.en_passant_target {
            hash ^= ZOBRIST.en_passant_keys[ep_square.file()];
        }

        hash
    }

    // =========================================================================
    // Make/Unmake helper methods
    // =========================================================================

    /// Remove a captured piece, updating board state, hash, and incremental eval.
    /// Returns the hash XOR delta for the capture.
    #[inline]
    fn remove_captured_piece(
        &mut self,
        capture_sq: Square,
        captured: (Color, Piece),
        opp_idx: usize,
    ) -> u64 {
        let (cap_col, cap_piece) = captured;
        let cap_sq_idx = capture_sq.index();
        let cap_p_idx = cap_piece.index();
        let cap_pst = pst_square(cap_sq_idx, cap_col == Color::White);

        // Remove from board
        self.remove_piece(capture_sq, cap_col, cap_piece);

        // Update incremental eval
        self.eval_mg[opp_idx] -= MATERIAL_MG[cap_p_idx] + PST_MG[cap_p_idx][cap_pst];
        self.eval_eg[opp_idx] -= MATERIAL_EG[cap_p_idx] + PST_EG[cap_p_idx][cap_pst];
        self.game_phase[opp_idx] -= PHASE_WEIGHTS[cap_p_idx];

        // Return hash delta
        ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)][color_to_zobrist_index(cap_col)]
            [square_to_zobrist_index(capture_sq)]
    }

    /// Execute castling: move king (already removed), place king and rook.
    /// Returns the hash XOR delta for the rook movement.
    #[inline]
    fn execute_castling(&mut self, m: &Move, color: Color, c_idx: usize, is_white: bool) -> u64 {
        let to_idx = m.to().index();
        let to_pst = pst_square(to_idx, is_white);

        // Place king at destination
        self.set_piece(m.to(), color, Piece::King);

        // Update eval for king placement (king index = 5)
        self.eval_mg[c_idx] += MATERIAL_MG[5] + PST_MG[5][to_pst];
        self.eval_eg[c_idx] += MATERIAL_EG[5] + PST_EG[5][to_pst];
        self.game_phase[c_idx] += PHASE_WEIGHTS[5];

        // Determine rook squares
        let (rook_from_f, rook_to_f) = if m.to().file() == 6 { (7, 5) } else { (0, 3) };
        let rook_from = Square::new(m.to().rank(), rook_from_f);
        let rook_to = Square::new(m.to().rank(), rook_to_f);
        let rook_from_idx = rook_from.index();
        let rook_to_idx = rook_to.index();

        // Move the rook
        let rook_info = self.piece_at(rook_from).expect("Castling without rook");
        self.remove_piece(rook_from, rook_info.0, rook_info.1);
        self.set_piece(rook_to, rook_info.0, rook_info.1);

        // Update eval for rook move (rook index = 3)
        let rook_from_pst = pst_square(rook_from_idx, is_white);
        let rook_to_pst = pst_square(rook_to_idx, is_white);
        self.eval_mg[c_idx] -= MATERIAL_MG[3] + PST_MG[3][rook_from_pst];
        self.eval_eg[c_idx] -= MATERIAL_EG[3] + PST_EG[3][rook_from_pst];
        self.eval_mg[c_idx] += MATERIAL_MG[3] + PST_MG[3][rook_to_pst];
        self.eval_eg[c_idx] += MATERIAL_EG[3] + PST_EG[3][rook_to_pst];

        // Return hash delta for rook movement
        ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)][color_to_zobrist_index(color)]
            [square_to_zobrist_index(rook_from)]
            ^ ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)][color_to_zobrist_index(color)]
                [square_to_zobrist_index(rook_to)]
    }

    /// Update castling rights based on a move.
    /// Returns the hash XOR delta for castling rights changes.
    #[inline]
    fn update_castling_rights(
        &mut self,
        m: &Move,
        moving_piece: Piece,
        color: Color,
        captured: Option<(Color, Piece)>,
    ) -> u64 {
        let mut hash_delta: u64 = 0;

        // King move removes both castling rights
        if moving_piece == Piece::King {
            if self.has_castling_right(color, 'K') {
                hash_delta ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                self.castling_rights &= !castle_bit(color, 'K');
            }
            if self.has_castling_right(color, 'Q') {
                hash_delta ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                self.castling_rights &= !castle_bit(color, 'Q');
            }
        } else if moving_piece == Piece::Rook {
            // Rook move from starting square removes that side's castling
            let start_rank = if color == Color::White { 0 } else { 7 };
            if m.from() == Square::new(start_rank, 0) && self.has_castling_right(color, 'Q') {
                hash_delta ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                self.castling_rights &= !castle_bit(color, 'Q');
            } else if m.from() == Square::new(start_rank, 7) && self.has_castling_right(color, 'K') {
                hash_delta ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                self.castling_rights &= !castle_bit(color, 'K');
            }
        }

        // Capturing a rook on its starting square removes opponent's castling
        if let Some((captured_color, captured_piece)) = captured {
            if captured_piece == Piece::Rook {
                let start_rank = if captured_color == Color::White { 0 } else { 7 };
                if m.to() == Square::new(start_rank, 0) && self.has_castling_right(captured_color, 'Q') {
                    hash_delta ^= ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][1];
                    self.castling_rights &= !castle_bit(captured_color, 'Q');
                } else if m.to() == Square::new(start_rank, 7)
                    && self.has_castling_right(captured_color, 'K')
                {
                    hash_delta ^= ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][0];
                    self.castling_rights &= !castle_bit(captured_color, 'K');
                }
            }
        }

        hash_delta
    }

    /// Remove captured piece for a move (including en passant) and return hash delta.
    fn capture_piece_for_move(
        &mut self,
        m: Move,
        is_white: bool,
        opp_idx: usize,
    ) -> (Option<(Color, Piece)>, u64) {
        if m.is_en_passant() {
            let capture_row = if is_white {
                m.to().rank() - 1
            } else {
                m.to().rank() + 1
            };
            let capture_sq = Square::new(capture_row, m.to().file());
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
    fn place_moving_piece(
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
            // execute_castling places king and rook plus eval updates
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

    /// Update en passant target based on the move and return hash delta.
    fn update_en_passant_target(&mut self, m: Move) -> u64 {
        self.en_passant_target = None;
        if m.is_double_pawn_push() {
            let ep_row = usize::midpoint(m.from().rank(), m.to().rank());
            let ep_sq = Square::new(ep_row, m.from().file());
            self.en_passant_target = Some(ep_sq);
            return ZOBRIST.en_passant_keys[ep_sq.file()];
        }
        0
    }

    /// Update halfmove clock after a move.
    fn update_halfmove_clock(&mut self, moving_piece: Piece, is_capture: bool) {
        if moving_piece == Piece::Pawn || is_capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        }
    }

    /// Record repetition info and return the previous count.
    fn record_repetition(&mut self, made_hash: u64) -> u32 {
        let previous_repetition_count = self.repetition_counts.get(made_hash);
        self.repetition_counts.increment(made_hash);
        previous_repetition_count
    }

    // =========================================================================
    // Core make/unmake implementation
    // =========================================================================

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

        let color = self.current_color();
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
        self.set_piece(Square::new(m.to().rank(), rook_orig_f), rook_info.0, rook_info.1);
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
            let capture_row = if color == Color::White {
                m.to().rank() - 1
            } else {
                m.to().rank() + 1
            };
            if let Some((cap_col, cap_piece)) = info.captured_piece_info {
                self.set_piece(Square::new(capture_row, m.to().file()), cap_col, cap_piece);
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

        let color = self.current_color();

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
