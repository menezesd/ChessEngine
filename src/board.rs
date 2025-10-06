use std::collections::HashSet;
use std::time::{Duration, Instant};
use crate::zobrist::{ZOBRIST, piece_to_zobrist_index, color_to_zobrist_index, square_to_zobrist_index};
use crate::types::{Color, Piece, Square, Move, UnmakeInfo, TranspositionTable, BoundType};
use crate::utils::mvv_lva_score;

#[derive(Clone, Debug)]
pub struct Board {
    pub squares: [[Option<(Color, Piece)>; 8]; 8],
    pub white_to_move: bool,
    pub en_passant_target: Option<Square>,
    pub castling_rights: HashSet<(Color, char)>,
    pub hash: u64,
}

impl Board {
    pub fn new() -> Self {
        let mut squares = [[None; 8]; 8];
        let back_rank = [
            Piece::Rook,
            Piece::Knight,
            Piece::Bishop,
            Piece::Queen,
            Piece::King,
            Piece::Bishop,
            Piece::Knight,
            Piece::Rook,
        ];
        for (i, piece) in back_rank.iter().enumerate() {
            squares[0][i] = Some((Color::White, *piece));
            squares[7][i] = Some((Color::Black, *piece));
            squares[1][i] = Some((Color::White, Piece::Pawn));
            squares[6][i] = Some((Color::Black, Piece::Pawn));
        }
        let mut castling_rights = HashSet::new();
        castling_rights.insert((Color::White, 'K'));
        castling_rights.insert((Color::White, 'Q'));
        castling_rights.insert((Color::Black, 'K'));
        castling_rights.insert((Color::Black, 'Q'));

        let mut board = Board {
            squares,
            white_to_move: true,
            en_passant_target: None,
            castling_rights,
            hash: 0,
        };
        board.hash = board.calculate_initial_hash();
        board
    }

    pub fn from_fen(fen: &str) -> Self {
        let mut squares = [[None; 8]; 8];
        let mut castling_rights = HashSet::new();
        let parts: Vec<&str> = fen.split_whitespace().collect();
        assert!(parts.len() >= 4, "FEN must have at least 4 parts");
        for (rank_idx, rank_str) in parts[0].split('/').enumerate() {
            let mut file = 0;
            for c in rank_str.chars() {
                if c.is_digit(10) {
                    file += c.to_digit(10).unwrap() as usize;
                } else {
                    let (color, piece) = match c {
                        'P' => (Color::White, Piece::Pawn),
                        'N' => (Color::White, Piece::Knight),
                        'B' => (Color::White, Piece::Bishop),
                        'R' => (Color::White, Piece::Rook),
                        'Q' => (Color::White, Piece::Queen),
                        'K' => (Color::White, Piece::King),
                        'p' => (Color::Black, Piece::Pawn),
                        'n' => (Color::Black, Piece::Knight),
                        'b' => (Color::Black, Piece::Bishop),
                        'r' => (Color::Black, Piece::Rook),
                        'q' => (Color::Black, Piece::Queen),
                        'k' => (Color::Black, Piece::King),
                        _ => panic!("Invalid piece char"),
                    };
                    squares[7 - rank_idx][file] = Some((color, piece));
                    file += 1;
                }
            }
        }
        let white_to_move = match parts[1] {
            "w" => true,
            "b" => false,
            _ => panic!("Invalid color"),
        };
        for c in parts[2].chars() {
            match c {
                'K' => { castling_rights.insert((Color::White, 'K')); }
                'Q' => { castling_rights.insert((Color::White, 'Q')); }
                'k' => { castling_rights.insert((Color::Black, 'K')); }
                'q' => { castling_rights.insert((Color::Black, 'Q')); }
                '-' => {}
                _ => panic!("Invalid castle"),
            }
        }
        let en_passant_target = if parts[3] != "-" {
            let chars: Vec<char> = parts[3].chars().collect();
            if chars.len() == 2 {
                Some(Square(crate::types::rank_to_index(chars[1]), crate::types::file_to_index(chars[0])))
            } else {
                None
            }
        } else {
            None
        };

        let mut board = Board { squares, white_to_move, en_passant_target, castling_rights, hash: 0 };
        board.hash = board.calculate_initial_hash();
        board
    }

    pub fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;
        for r in 0..8 {
            for f in 0..8 {
                if let Some((color, piece)) = self.squares[r][f] {
                    let sq_idx = square_to_zobrist_index(Square(r, f));
                    let p_idx = piece_to_zobrist_index(piece);
                    let c_idx = color_to_zobrist_index(color);
                    hash ^= ZOBRIST.piece_keys[p_idx][c_idx][sq_idx];
                }
            }
        }
        if !self.white_to_move { hash ^= ZOBRIST.black_to_move_key; }
        if self.castling_rights.contains(&(Color::White, 'K')) { hash ^= ZOBRIST.castling_keys[0][0]; }
        if self.castling_rights.contains(&(Color::White, 'Q')) { hash ^= ZOBRIST.castling_keys[0][1]; }
        if self.castling_rights.contains(&(Color::Black, 'K')) { hash ^= ZOBRIST.castling_keys[1][0]; }
        if self.castling_rights.contains(&(Color::Black, 'Q')) { hash ^= ZOBRIST.castling_keys[1][1]; }
        if let Some(ep) = self.en_passant_target { hash ^= ZOBRIST.en_passant_keys[ep.1]; }
        hash
    }

    pub fn make_move(&mut self, m: &Move) -> UnmakeInfo {
        let mut current_hash = self.hash;
        let previous_hash = self.hash;

        let color = self.current_color();

        let previous_en_passant_target = self.en_passant_target;
        let previous_castling_rights = self.castling_rights.clone();

        current_hash ^= ZOBRIST.black_to_move_key;
        if let Some(old_ep) = self.en_passant_target {
            current_hash ^= ZOBRIST.en_passant_keys[old_ep.1];
        }

        let mut captured_piece_info: Option<(Color, Piece)> = None;
        let mut captured_sq_idx: Option<usize> = None;

        if m.is_en_passant {
            let capture_row = if color == Color::White { m.to.0 - 1 } else { m.to.0 + 1 };
            let capture_sq = Square(capture_row, m.to.1);
            captured_sq_idx = Some(square_to_zobrist_index(capture_sq));
            captured_piece_info = self.squares[capture_row][m.to.1];
            self.squares[capture_row][m.to.1] = None;
            if let Some((cap_col, cap_piece)) = captured_piece_info {
                current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                    [color_to_zobrist_index(cap_col)][captured_sq_idx.unwrap()];
            }
        } else if !m.is_castling {
            captured_piece_info = self.squares[m.to.0][m.to.1];
            if captured_piece_info.is_some() {
                captured_sq_idx = Some(square_to_zobrist_index(m.to));
                if let Some((cap_col, cap_piece)) = captured_piece_info {
                    current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)]
                        [color_to_zobrist_index(cap_col)][captured_sq_idx.unwrap()];
                }
            }
        }

        let moving_piece_info = self.squares[m.from.0][m.from.1].expect("make_move 'from' empty");
        let (moving_color, moving_piece) = moving_piece_info;
        let from_sq_idx = square_to_zobrist_index(m.from);
        let to_sq_idx = square_to_zobrist_index(m.to);

        current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(moving_piece)]
            [color_to_zobrist_index(moving_color)][from_sq_idx];
        self.squares[m.from.0][m.from.1] = None;

        if m.is_castling {
            self.squares[m.to.0][m.to.1] = Some((color, Piece::King));
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::King)]
                [color_to_zobrist_index(color)][to_sq_idx];
            let (rook_from_f, rook_to_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_from_sq = Square(m.to.0, rook_from_f);
            let rook_to_sq = Square(m.to.0, rook_to_f);
            let rook_info = self.squares[rook_from_sq.0][rook_from_sq.1].expect("Castling without rook");
            self.squares[rook_from_sq.0][rook_from_sq.1] = None;
            self.squares[rook_to_sq.0][rook_to_sq.1] = Some(rook_info);
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
            self.squares[m.to.0][m.to.1] = Some(piece_to_place);
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

        let mut new_castling_rights = self.castling_rights.clone();
        let mut castle_hash_diff: u64 = 0;
        if moving_piece == Piece::King {
            if self.castling_rights.contains(&(color, 'K')) {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                new_castling_rights.remove(&(color, 'K'));
            }
            if self.castling_rights.contains(&(color, 'Q')) {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                new_castling_rights.remove(&(color, 'Q'));
            }
        } else if moving_piece == Piece::Rook {
            let start_rank = if color == Color::White { 0 } else { 7 };
            if m.from == Square(start_rank, 0) && self.castling_rights.contains(&(color, 'Q')) {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1];
                new_castling_rights.remove(&(color, 'Q'));
            } else if m.from == Square(start_rank, 7)
                && self.castling_rights.contains(&(color, 'K'))
            {
                castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0];
                new_castling_rights.remove(&(color, 'K'));
            }
        }
        if let Some((captured_color, captured_piece)) = captured_piece_info {
            if captured_piece == Piece::Rook {
                let start_rank = if captured_color == Color::White { 0 } else { 7 };
                if m.to == Square(start_rank, 0)
                    && self.castling_rights.contains(&(captured_color, 'Q'))
                {
                    castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][1];
                    new_castling_rights.remove(&(captured_color, 'Q'));
                } else if m.to == Square(start_rank, 7)
                    && self.castling_rights.contains(&(captured_color, 'K'))
                {
                    castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][0];
                    new_castling_rights.remove(&(captured_color, 'K'));
                }
            }
        }
        self.castling_rights = new_castling_rights;
        current_hash ^= castle_hash_diff;

        self.white_to_move = !self.white_to_move;
        self.hash = current_hash;

        UnmakeInfo {
            captured_piece_info,
            previous_en_passant_target,
            previous_castling_rights,
            previous_hash,
        }
    }

    pub fn unmake_move(&mut self, m: &Move, info: UnmakeInfo) {
        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash;

        let color = self.current_color();

        let piece_that_moved = if m.promotion.is_some() {
            (color, Piece::Pawn)
        } else if m.is_castling {
            (color, Piece::King)
        } else {
            self.squares[m.to.0][m.to.1].expect("Unmake move: 'to' square empty?")
        };

        if m.is_castling {
            self.squares[m.from.0][m.from.1] = Some(piece_that_moved);
            self.squares[m.to.0][m.to.1] = None;

            let (rook_orig_f, rook_moved_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_info = self.squares[m.to.0][rook_moved_f].expect("Unmake castling: rook missing");
            self.squares[m.to.0][rook_moved_f] = None;
            self.squares[m.to.0][rook_orig_f] = Some(rook_info);
        } else {
            self.squares[m.from.0][m.from.1] = Some(piece_that_moved);

            if m.is_en_passant {
                self.squares[m.to.0][m.to.1] = None;
                let capture_row = if color == Color::White { m.to.0 - 1 } else { m.to.0 + 1 };
                self.squares[capture_row][m.to.1] = info.captured_piece_info;
            } else {
                self.squares[m.to.0][m.to.1] = info.captured_piece_info;
            }
        }
    }

    pub fn generate_moves(&mut self) -> Vec<Move> {
        // ...existing generate_moves code...
        unimplemented!()
    }

    // ... Other Board methods omitted for brevity ...
}
