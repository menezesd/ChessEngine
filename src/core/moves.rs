use crate::core::board::{Board, UnmakeInfo, NullUnmake};
use crate::core::types::{Move, Color, Piece, Square};
use crate::core::zobrist::{color_to_zobrist_index, piece_to_zobrist_index, square_to_zobrist_index, ZOBRIST};
use crate::core::bitboard::castling_bit;
use crate::core::constants::*;

/// Castling rook and king file positions
const KINGSIDE_ROOK_FILE: usize = 7;
const QUEENSIDE_ROOK_FILE: usize = 0;
const KINGSIDE_KING_FILE: usize = 6;
const KINGSIDE_ROOK_DEST_FILE: usize = 5;
const QUEENSIDE_ROOK_DEST_FILE: usize = 3;

impl Board {
    /// Make a move on the board, updating internal state and returning the
    /// minimal snapshot required to restore the previous position.
    ///
    /// The returned `UnmakeInfo` must be passed to `unmake_move` to restore
    /// the board to its previous state. This function updates the zobrist
    /// `hash`, halfmove clock, castling rights, en-passant target and the
    /// internal piece bitboards.
    pub fn make_move(&mut self, m: &Move) -> UnmakeInfo {
        let mut current_hash = self.hash;
        let previous_hash = self.hash;
        let previous_halfmove_clock = self.halfmove_clock;
        let previous_history_len = self.position_history.len();
        let color = self.current_color();

        let previous_en_passant_target = self.en_passant_target;
        let previous_castling_rights = self.castling_rights;

        current_hash ^= ZOBRIST.black_to_move_key;

        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= ZOBRIST.en_passant_keys[old_ep.1];
        }

        let mut captured_piece_info: Option<(Color, Piece)> = None;
        if m.is_en_passant {
            captured_piece_info = self.handle_en_passant_capture(m, &mut current_hash);
        } else if !m.is_castling {
            captured_piece_info = self.get_square(m.to.0, m.to.1);
            if let Some((cap_col, cap_piece)) = captured_piece_info {
                let capture_idx = square_to_zobrist_index(m.to);
                current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                    [color_to_zobrist_index(cap_col)][capture_idx];
            }
        }

        let moving_piece_info = self
            .get_square(m.from.0, m.from.1)
            .expect("make_move 'from' empty");
        let (moving_color, moving_piece) = moving_piece_info;
        let from_sq_idx = square_to_zobrist_index(m.from);
        let to_sq_idx = square_to_zobrist_index(m.to);

        current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(moving_piece)]
            [color_to_zobrist_index(moving_color)][from_sq_idx];

        self.set_square(m.from.0, m.from.1, None);

        if m.is_castling {
            self.execute_castling(m, &mut current_hash);
        } else {
            let piece_to_place = if let Some(promoted_piece) = m.promotion {
                (color, promoted_piece)
            } else {
                moving_piece_info
            };
            self.set_square(m.to.0, m.to.1, Some(piece_to_place));
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(piece_to_place.1)]
                [color_to_zobrist_index(piece_to_place.0)][to_sq_idx];
        }

        self.update_en_passant_target(m, moving_piece, &mut current_hash);
        let castle_hash_diff = self.update_castling_rights(m, moving_piece, color, captured_piece_info);
        current_hash ^= castle_hash_diff;

        self.white_to_move = !self.white_to_move;
        self.hash = current_hash;

        // Update halfmove clock
        if moving_piece == Piece::Pawn || captured_piece_info.is_some() {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        }

        // Update position history
        self.position_history.push(self.hash);

        UnmakeInfo {
            captured_piece_info,
            previous_en_passant_target,
            previous_castling_rights,
            previous_hash,
            previous_halfmove_clock,
            previous_history_len,
        }
    }

    /// Restore a previously-made move using the `UnmakeInfo` returned by
    /// `make_move`.
    ///
    /// This restores the board state (including `hash`, clocks and pieces)
    /// exactly to the state before the corresponding `make_move` call.
    pub fn unmake_move(&mut self, m: &Move, info: UnmakeInfo) {
        // Restore state directly from info
        self.white_to_move = !self.white_to_move; // Switch turn back first
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash; // Restore hash directly!

        // Restore halfmove clock and position history
        self.halfmove_clock = info.previous_halfmove_clock;
        self.position_history
            .truncate(info.previous_history_len);

        // Restore pieces on board (no hash updates needed here as hash is fully restored)
        let color = self.current_color();

        let piece_that_moved = if m.promotion.is_some() {
            (color, Piece::Pawn)
        } else if m.is_castling {
            (color, Piece::King) // Assume king if castling
        } else {
            self.get_square(m.to.0, m.to.1)
                .expect("Unmake move: 'to' square empty?")
        };

        if m.is_castling {
            self.undo_castling(m);
        } else {
            self.set_square(m.from.0, m.from.1, Some(piece_that_moved));

            if m.is_en_passant {
                self.set_square(m.to.0, m.to.1, None);
                let capture_row = if color == Color::White {
                    m.to.0 - 1
                } else {
                    m.to.0 + 1
                };
                self.set_square(capture_row, m.to.1, info.captured_piece_info);
            } else {
                // Regular move: Put back captured piece (or None)
                self.set_square(m.to.0, m.to.1, info.captured_piece_info);
            }
        }
    }

    /// Make a "null move" (pass the turn) for null-move pruning.
    /// Returns a NullUnmake snapshot to restore position.
    pub fn make_null_move(&mut self) -> NullUnmake {
        let previous_en_passant_target = self.en_passant_target;
        let previous_hash = self.hash;
        let previous_halfmove_clock = self.halfmove_clock;
        let previous_history_len = self.position_history.len();

        // Update side to move and zobrist hash accordingly
        self.white_to_move = !self.white_to_move;
        self.hash ^= ZOBRIST.black_to_move_key;

        // Null move increments halfmove clock
        self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        self.en_passant_target = None;
        self.position_history.push(self.hash);

        NullUnmake {
            previous_en_passant_target,
            previous_hash,
            previous_halfmove_clock,
            previous_history_len,
        }
    }

    /// Restore state after a null move using the provided snapshot.
    pub fn unmake_null_move(&mut self, info: NullUnmake) {
        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.hash = info.previous_hash;
        self.halfmove_clock = info.previous_halfmove_clock;
        self.position_history.truncate(info.previous_history_len);
    }

    /// Extract en-passant capture logic for reuse
    fn handle_en_passant_capture(&mut self, m: &Move, current_hash: &mut u64) -> Option<(Color, Piece)> {
        let capture_row = if self.current_color() == Color::White {
            m.to.0 - 1
        } else {
            m.to.0 + 1
        };
        let capture_sq = Square(capture_row, m.to.1);
        let capture_idx = square_to_zobrist_index(capture_sq);
        let captured_piece_info = self.get_square(capture_row, m.to.1);
        self.set_square(capture_row, m.to.1, None);

        if let Some((cap_col, cap_piece)) = captured_piece_info {
            *current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                [color_to_zobrist_index(cap_col)][capture_idx];
        }
        captured_piece_info
    }

    /// Extract en-passant target setting logic for reuse
    fn update_en_passant_target(&mut self, m: &Move, moving_piece: Piece, current_hash: &mut u64) {
        self.en_passant_target = None;
        if moving_piece == Piece::Pawn && (m.from.0 as isize - m.to.0 as isize).abs() == 2 {
            let ep_row = (m.from.0 + m.to.0) / 2;
            let ep_sq = Square(ep_row, m.from.1);
            self.en_passant_target = Some(ep_sq);
            *current_hash ^= ZOBRIST.en_passant_keys[ep_sq.1];
        }
    }

    /// Execute castling move
    fn execute_castling(&mut self, m: &Move, current_hash: &mut u64) {
        let color = self.current_color();
        self.set_square(m.to.0, m.to.1, Some((color, Piece::King)));
        *current_hash ^= ZOBRIST.piece_keys[KING_INDEX][color_to_zobrist_index(color)][square_to_zobrist_index(m.to)];

        let (rook_from_f, rook_to_f) = if m.to.1 == KINGSIDE_KING_FILE {
            (KINGSIDE_ROOK_FILE, KINGSIDE_ROOK_DEST_FILE)
        } else {
            (QUEENSIDE_ROOK_FILE, QUEENSIDE_ROOK_DEST_FILE)
        };
        let rook_from_sq = Square(m.to.0, rook_from_f);
        let rook_to_sq = Square(m.to.0, rook_to_f);
        let rook_info = self
            .get_square(rook_from_sq.0, rook_from_sq.1)
            .expect("Castling without rook");
        self.set_square(rook_from_sq.0, rook_from_sq.1, None);
        self.set_square(rook_to_sq.0, rook_to_sq.1, Some(rook_info));

        *current_hash ^= ZOBRIST.piece_keys[ROOK_INDEX][color_to_zobrist_index(color)][square_to_zobrist_index(rook_from_sq)];
        *current_hash ^= ZOBRIST.piece_keys[ROOK_INDEX][color_to_zobrist_index(color)][square_to_zobrist_index(rook_to_sq)];
    }

    /// Undo castling move
    fn undo_castling(&mut self, m: &Move) {
        let color = self.current_color();
        self.set_square(m.from.0, m.from.1, Some((color, Piece::King)));
        self.set_square(m.to.0, m.to.1, None);

        let (rook_orig_f, rook_moved_f) = if m.to.1 == KINGSIDE_KING_FILE {
            (KINGSIDE_ROOK_FILE, KINGSIDE_ROOK_DEST_FILE)
        } else {
            (QUEENSIDE_ROOK_FILE, QUEENSIDE_ROOK_DEST_FILE)
        };
        let rook_info = self
            .get_square(m.to.0, rook_moved_f)
            .expect("Unmake castling: rook missing");
        self.set_square(m.to.0, rook_moved_f, None);
        self.set_square(m.to.0, rook_orig_f, Some(rook_info));
    }

    /// Update castling rights and return hash difference
    fn update_castling_rights(&mut self, m: &Move, moving_piece: Piece, color: Color, captured_piece_info: Option<(Color, Piece)>) -> u64 {
        let mut castle_hash_diff: u64 = 0;

        if moving_piece == Piece::King {
            if self.castling_rights & castling_bit(color, 'K') != 0 {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                self.castling_rights &= !castling_bit(color, 'K');
            }
            if self.castling_rights & castling_bit(color, 'Q') != 0 {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                self.castling_rights &= !castling_bit(color, 'Q');
            }
        } else if moving_piece == Piece::Rook {
            let start_rank = if color == Color::White { WHITE_START_RANK } else { BLACK_START_RANK };
            if m.from == Square(start_rank, KINGSIDE_ROOK_FILE)
                && self.castling_rights & castling_bit(color, 'K') != 0
            {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                self.castling_rights &= !castling_bit(color, 'K');
            } else if m.from == Square(start_rank, QUEENSIDE_ROOK_FILE)
                && self.castling_rights & castling_bit(color, 'Q') != 0
            {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                self.castling_rights &= !castling_bit(color, 'Q');
            }
        }

        if let Some((captured_color, captured_piece)) = captured_piece_info {
            if captured_piece == Piece::Rook {
                let start_rank = if captured_color == Color::White { WHITE_START_RANK } else { BLACK_START_RANK };
                if m.to == Square(start_rank, KINGSIDE_ROOK_FILE)
                    && self.castling_rights & castling_bit(captured_color, 'K') != 0
                {
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][0];
                    self.castling_rights &= !castling_bit(captured_color, 'K');
                } else if m.to == Square(start_rank, QUEENSIDE_ROOK_FILE)
                    && self.castling_rights & castling_bit(captured_color, 'Q') != 0
                {
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][1];
                    self.castling_rights &= !castling_bit(captured_color, 'Q');
                }
            }
        }
        castle_hash_diff
    }
}