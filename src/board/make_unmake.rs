use crate::zobrist::{
    color_to_zobrist_index, piece_to_zobrist_index, square_to_zobrist_index, ZOBRIST,
};

use super::{
    bit_for_square, castle_bit, color_index, piece_index, Board, Color, Move, NullMoveInfo, Piece,
    Square, UnmakeInfo,
};

impl Board {
    pub(crate) fn current_color(&self) -> Color {
        if self.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    pub(crate) fn opponent_color(&self, color: Color) -> Color {
        match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    pub(crate) fn has_castling_right(&self, color: Color, side: char) -> bool {
        self.castling_rights & castle_bit(color, side) != 0
    }

    pub(crate) fn set_piece(&mut self, sq: Square, color: Color, piece: Piece) {
        let bit = bit_for_square(sq).0;
        let c_idx = color_index(color);
        let p_idx = piece_index(piece);
        self.pieces[c_idx][p_idx].0 |= bit;
        self.occupied[c_idx].0 |= bit;
        self.all_occupied.0 |= bit;
    }

    pub(crate) fn remove_piece(&mut self, sq: Square, color: Color, piece: Piece) {
        let bit = bit_for_square(sq).0;
        let c_idx = color_index(color);
        let p_idx = piece_index(piece);
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
        let c_idx = color_index(color);
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

    pub(crate) fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;

        for r in 0..8 {
            for f in 0..8 {
                let sq = Square(r, f);
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
            hash ^= ZOBRIST.en_passant_keys[ep_square.1];
        }

        hash
    }

    pub(crate) fn make_move(&mut self, m: &Move) -> UnmakeInfo {
        let mut current_hash = self.hash;
        let previous_hash = self.hash;

        let color = self.current_color();

        let previous_en_passant_target = self.en_passant_target;
        let previous_castling_rights = self.castling_rights;
        let previous_halfmove_clock = self.halfmove_clock;

        current_hash ^= ZOBRIST.black_to_move_key;

        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= ZOBRIST.en_passant_keys[old_ep.1];
        }

        let mut captured_piece_info: Option<(Color, Piece)> = None;

        if m.is_en_passant {
            let capture_row = if color == Color::White {
                m.to.0 - 1
            } else {
                m.to.0 + 1
            };
            let capture_sq = Square(capture_row, m.to.1);
            captured_piece_info = self.piece_at(capture_sq);
            if let Some((cap_col, cap_piece)) = captured_piece_info {
                self.remove_piece(capture_sq, cap_col, cap_piece);
                current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                    [color_to_zobrist_index(cap_col)][square_to_zobrist_index(capture_sq)];
            }
        } else if !m.is_castling {
            captured_piece_info = self.piece_at(m.to);
            if let Some((cap_col, cap_piece)) = captured_piece_info {
                self.remove_piece(m.to, cap_col, cap_piece);
                current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                    [color_to_zobrist_index(cap_col)][square_to_zobrist_index(m.to)];
            }
        }

        let moving_piece_info = self.piece_at(m.from).expect("make_move 'from' empty");
        let (moving_color, moving_piece) = moving_piece_info;
        let from_sq_idx = square_to_zobrist_index(m.from);
        let to_sq_idx = square_to_zobrist_index(m.to);

        current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(moving_piece)]
            [color_to_zobrist_index(moving_color)][from_sq_idx];

        self.remove_piece(m.from, moving_color, moving_piece);

        if m.is_castling {
            self.set_piece(m.to, color, Piece::King);
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::King)]
                [color_to_zobrist_index(color)][to_sq_idx];

            let (rook_from_f, rook_to_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_from_sq = Square(m.to.0, rook_from_f);
            let rook_to_sq = Square(m.to.0, rook_to_f);
            let rook_info = self.piece_at(rook_from_sq).expect("Castling without rook");
            self.remove_piece(rook_from_sq, rook_info.0, rook_info.1);
            self.set_piece(rook_to_sq, rook_info.0, rook_info.1);

            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)]
                [color_to_zobrist_index(color)][square_to_zobrist_index(rook_from_sq)];
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)]
                [color_to_zobrist_index(color)][square_to_zobrist_index(rook_to_sq)];
        } else {
            let piece_to_place = if let Some(promoted_piece) = m.promotion {
                (color, promoted_piece)
            } else {
                moving_piece_info
            };
            self.set_piece(m.to, piece_to_place.0, piece_to_place.1);
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(piece_to_place.1)]
                [color_to_zobrist_index(piece_to_place.0)][to_sq_idx];
        }

        self.en_passant_target = None;
        if moving_piece == Piece::Pawn && (m.from.0 as isize - m.to.0 as isize).abs() == 2 {
            let ep_row = (m.from.0 + m.to.0) / 2;
            let ep_sq = Square(ep_row, m.from.1);
            self.en_passant_target = Some(ep_sq);
            current_hash ^= ZOBRIST.en_passant_keys[ep_sq.1];
        }

        let is_capture = m.captured_piece.is_some() || m.is_en_passant;
        if moving_piece == Piece::Pawn || is_capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        }

        let mut castle_hash_diff: u64 = 0;

        if moving_piece == Piece::King {
            if self.has_castling_right(color, 'K') {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                self.castling_rights &= !castle_bit(color, 'K');
            }
            if self.has_castling_right(color, 'Q') {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                self.castling_rights &= !castle_bit(color, 'Q');
            }
        } else if moving_piece == Piece::Rook {
            let start_rank = if color == Color::White { 0 } else { 7 };
            if m.from == Square(start_rank, 0) && self.has_castling_right(color, 'Q') {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                self.castling_rights &= !castle_bit(color, 'Q');
            } else if m.from == Square(start_rank, 7) && self.has_castling_right(color, 'K') {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                self.castling_rights &= !castle_bit(color, 'K');
            }
        }

        if let Some((captured_color, captured_piece)) = captured_piece_info {
            if captured_piece == Piece::Rook {
                let start_rank = if captured_color == Color::White { 0 } else { 7 };
                if m.to == Square(start_rank, 0) && self.has_castling_right(captured_color, 'Q') {
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][1];
                    self.castling_rights &= !castle_bit(captured_color, 'Q');
                } else if m.to == Square(start_rank, 7)
                    && self.has_castling_right(captured_color, 'K')
                {
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][0];
                    self.castling_rights &= !castle_bit(captured_color, 'K');
                }
            }
        }
        current_hash ^= castle_hash_diff;

        self.white_to_move = !self.white_to_move;
        self.hash = current_hash;

        let made_hash = current_hash;
        let previous_repetition_count = self.repetition_counts.get(made_hash);
        self.repetition_counts.increment(made_hash);

        UnmakeInfo {
            captured_piece_info,
            previous_en_passant_target,
            previous_castling_rights,
            previous_hash,
            previous_halfmove_clock,
            made_hash,
            previous_repetition_count,
        }
    }

    pub(crate) fn make_null_move(&mut self) -> NullMoveInfo {
        let previous_hash = self.hash;
        let previous_en_passant_target = self.en_passant_target;
        let mut current_hash = self.hash;

        current_hash ^= ZOBRIST.black_to_move_key;
        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= ZOBRIST.en_passant_keys[old_ep.1];
        }
        self.en_passant_target = None;
        self.white_to_move = !self.white_to_move;
        self.hash = current_hash;

        NullMoveInfo {
            previous_en_passant_target,
            previous_hash,
        }
    }

    pub(crate) fn unmake_move(&mut self, m: &Move, info: UnmakeInfo) {
        self.repetition_counts
            .set(info.made_hash, info.previous_repetition_count);

        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash;
        self.halfmove_clock = info.previous_halfmove_clock;

        let color = self.current_color();

        let piece_that_moved = if m.promotion.is_some() {
            (color, Piece::Pawn)
        } else if m.is_castling {
            (color, Piece::King)
        } else {
            self.piece_at(m.to)
                .expect("Unmake move: 'to' square empty?")
        };

        if m.is_castling {
            self.set_piece(m.from, piece_that_moved.0, piece_that_moved.1);
            self.remove_piece(m.to, color, Piece::King);

            let (rook_orig_f, rook_moved_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_sq = Square(m.to.0, rook_moved_f);
            let rook_info = self
                .piece_at(rook_sq)
                .expect("Unmake castling: rook missing");
            self.remove_piece(rook_sq, rook_info.0, rook_info.1);
            self.set_piece(Square(m.to.0, rook_orig_f), rook_info.0, rook_info.1);
        } else {
            let moved_piece_at_to = self
                .piece_at(m.to)
                .expect("Unmake move: 'to' square empty?");
            self.remove_piece(m.to, moved_piece_at_to.0, moved_piece_at_to.1);
            let piece_on_from = if m.promotion.is_some() {
                (color, Piece::Pawn)
            } else {
                moved_piece_at_to
            };
            self.set_piece(m.from, piece_on_from.0, piece_on_from.1);

            if m.is_en_passant {
                let capture_row = if color == Color::White {
                    m.to.0 - 1
                } else {
                    m.to.0 + 1
                };
                if let Some((cap_col, cap_piece)) = info.captured_piece_info {
                    self.set_piece(Square(capture_row, m.to.1), cap_col, cap_piece);
                }
            } else if let Some((cap_col, cap_piece)) = info.captured_piece_info {
                self.set_piece(m.to, cap_col, cap_piece);
            }
        }
    }

    pub(crate) fn unmake_null_move(&mut self, info: NullMoveInfo) {
        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.hash = info.previous_hash;
    }
}
