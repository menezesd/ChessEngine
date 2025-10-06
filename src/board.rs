use std::collections::HashSet;
use crate::types::*;
use crate::zobrist::*;
use crate::utils::format_square;

pub fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 300,
        Piece::Bishop => 300,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 10000, // Usually not used in MVV-LVA since king captures are illegal
    }
}

pub fn mvv_lva_score(m: &Move, board: &Board) -> i32 {
    if let Some(victim) = m.captured_piece {
        let attacker = board.squares[m.from.0][m.from.1].unwrap().1;
        let victim_value = piece_value(victim);
        let attacker_value = piece_value(attacker);
        victim_value * 10 - attacker_value // prioritize more valuable victims, less valuable attackers
    } else {
        0 // Non-captures get low priority
    }
}

#[derive(Clone, Debug)]
pub struct Board {
    pub squares: [[Option<(Color, Piece)>; 8]; 8],
    pub white_to_move: bool,
    pub en_passant_target: Option<Square>,
    pub castling_rights: HashSet<(Color, char)>, // 'K' or 'Q'
    pub hash: u64,                               // Add Zobrist hash field
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
                'K' => {
                    castling_rights.insert((Color::White, 'K'));
                }
                'Q' => {
                    castling_rights.insert((Color::White, 'Q'));
                }
                'k' => {
                    castling_rights.insert((Color::Black, 'K'));
                }
                'q' => {
                    castling_rights.insert((Color::Black, 'Q'));
                }
                '-' => {}
                _ => panic!("Invalid castle"),
            }
        }
        let en_passant_target = if parts[3] != "-" {
            let chars: Vec<char> = parts[3].chars().collect();
            if chars.len() == 2 {
                Some(Square(crate::utils::rank_to_index(chars[1]), crate::utils::file_to_index(chars[0])))
            } else {
                None
            }
        } else {
            None
        };

        let mut board = Board {
            squares,
            white_to_move,
            en_passant_target,
            castling_rights,
            hash: 0,
        };
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

        if !self.white_to_move {
            hash ^= ZOBRIST.black_to_move_key;
        }

        if self.castling_rights.contains(&(Color::White, 'K')) {
            hash ^= ZOBRIST.castling_keys[0][0];
        }
        if self.castling_rights.contains(&(Color::White, 'Q')) {
            hash ^= ZOBRIST.castling_keys[0][1];
        }
        if self.castling_rights.contains(&(Color::Black, 'K')) {
            hash ^= ZOBRIST.castling_keys[1][0];
        }
        if self.castling_rights.contains(&(Color::Black, 'Q')) {
            hash ^= ZOBRIST.castling_keys[1][1];
        }

        if let Some(ep_square) = self.en_passant_target {
            hash ^= ZOBRIST.en_passant_keys[ep_square.1];
        }

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
            let capture_row = if color == Color::White {
                m.to.0 - 1
            } else {
                m.to.0 + 1
            };
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
            let rook_info =
                self.squares[rook_from_sq.0][rook_from_sq.1].expect("Castling without rook");
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
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][1];
                    new_castling_rights.remove(&(captured_color, 'Q'));
                } else if m.to == Square(start_rank, 7)
                    && self.castling_rights.contains(&(captured_color, 'K'))
                {
                    castle_hash_diff ^=
                        ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][0];
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
            let rook_info =
                self.squares[m.to.0][rook_moved_f].expect("Unmake castling: rook missing");
            self.squares[m.to.0][rook_moved_f] = None;
            self.squares[m.to.0][rook_orig_f] = Some(rook_info);
        } else {
            self.squares[m.from.0][m.from.1] = Some(piece_that_moved);

            if m.is_en_passant {
                self.squares[m.to.0][m.to.1] = None;
                let capture_row = if color == Color::White {
                    m.to.0 - 1
                } else {
                    m.to.0 + 1
                };
                self.squares[capture_row][m.to.1] = info.captured_piece_info;
            } else {
                self.squares[m.to.0][m.to.1] = info.captured_piece_info;
            }
        }
    }

    pub fn generate_pseudo_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        let color = if self.white_to_move {
            Color::White
        } else {
            Color::Black
        };

        for rank in 0..8 {
            for file in 0..8 {
                if let Some((c, piece)) = self.squares[rank][file] {
                    if c == color {
                        let from = Square(rank, file);
                        moves.extend(self.generate_piece_moves(from, piece));
                    }
                }
            }
        }
        moves
    }

    pub fn generate_piece_moves(&self, from: Square, piece: Piece) -> Vec<Move> {
        match piece {
            Piece::Pawn => self.generate_pawn_moves(from),
            Piece::Knight => self.generate_knight_moves(from),
            Piece::Bishop => {
                self.generate_sliding_moves(from, &[(1, 1), (1, -1), (-1, 1), (-1, -1)])
            }
            Piece::Rook => self.generate_sliding_moves(from, &[(1, 0), (-1, 0), (0, 1), (0, -1)]),
            Piece::Queen => self.generate_sliding_moves(
                from,
                &[
                    (1, 0),
                    (-1, 0),
                    (0, 1),
                    (0, -1),
                    (1, 1),
                    (1, -1),
                    (-1, 1),
                    (-1, -1),
                ],
            ),
            Piece::King => self.generate_king_moves(from),
        }
    }

    fn create_move(
        &self,
        from: Square,
        to: Square,
        promotion: Option<Piece>,
        is_castling: bool,
        is_en_passant: bool,
    ) -> Move {
        let captured_piece = if is_en_passant {
            Some(Piece::Pawn)
        } else if !is_castling {
            self.squares[to.0][to.1].map(|(_, p)| p)
        } else {
            None
        };

        Move {
            from,
            to,
            promotion,
            is_castling,
            is_en_passant,
            captured_piece,
        }
    }

    fn generate_pawn_moves(&self, from: Square) -> Vec<Move> {
        let color = if self.white_to_move {
            Color::White
        } else {
            Color::Black
        };
        let mut moves = Vec::new();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let start_rank = if color == Color::White { 1 } else { 6 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        let r = from.0 as isize;
        let f = from.1 as isize;

        let forward_r = r + dir;
        if forward_r >= 0 && forward_r < 8 {
            let forward_sq = Square(forward_r as usize, f as usize);
            if self.squares[forward_sq.0][forward_sq.1].is_none() {
                if forward_sq.0 == promotion_rank {
                    for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                        moves.push(self.create_move(from, forward_sq, Some(promo), false, false));
                    }
                } else {
                    moves.push(self.create_move(from, forward_sq, None, false, false));
                    if r == start_rank as isize {
                        let double_forward_r = r + 2 * dir;
                        let double_forward_sq = Square(double_forward_r as usize, f as usize);
                        if self.squares[double_forward_sq.0][double_forward_sq.1].is_none() {
                            moves.push(self.create_move(
                                from,
                                double_forward_sq,
                                None,
                                false,
                                false,
                            ));
                        }
                    }
                }
            }
        }

        if forward_r >= 0 && forward_r < 8 {
            for df in [-1, 1] {
                let capture_f = f + df;
                if capture_f >= 0 && capture_f < 8 {
                    let target_sq = Square(forward_r as usize, capture_f as usize);
                    if let Some((target_color, _)) = self.squares[target_sq.0][target_sq.1] {
                        if target_color != color {
                            if target_sq.0 == promotion_rank {
                                for promo in
                                    [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
                                {
                                    moves.push(self.create_move(
                                        from,
                                        target_sq,
                                        Some(promo),
                                        false,
                                        false,
                                    ));
                                }
                            } else {
                                moves.push(self.create_move(from, target_sq, None, false, false));
                            }
                        }
                    } else if Some(target_sq) == self.en_passant_target {
                        moves.push(self.create_move(from, target_sq, None, false, true));
                    }
                }
            }
        }

        moves
    }

    fn generate_knight_moves(&self, from: Square) -> Vec<Move> {
        let mut moves = Vec::new();
        let deltas = [
            (2, 1),
            (1, 2),
            (-1, 2),
            (-2, 1),
            (-2, -1),
            (-1, -2),
            (1, -2),
            (2, -1),
        ];
        let (rank, file) = (from.0 as isize, from.1 as isize);
        let color = self.current_color();

        for (dr, df) in deltas {
            let (nr, nf) = (rank + dr, file + df);
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                let to_sq = Square(nr as usize, nf as usize);
                if let Some((c, _)) = self.squares[to_sq.0][to_sq.1] {
                    if c != color {
                        moves.push(self.create_move(from, to_sq, None, false, false));
                    }
                } else {
                    moves.push(self.create_move(from, to_sq, None, false, false));
                }
            }
        }
        moves
    }

    fn generate_sliding_moves(&self, from: Square, directions: &[(isize, isize)]) -> Vec<Move> {
        let mut moves = Vec::new();
        let (rank, file) = (from.0 as isize, from.1 as isize);
        let color = self.current_color();

        for &(dr, df) in directions {
            let mut r = rank + dr;
            let mut f = file + df;
            while r >= 0 && r < 8 && f >= 0 && f < 8 {
                let to_sq = Square(r as usize, f as usize);
                if let Some((c, _)) = self.squares[to_sq.0][to_sq.1] {
                    if c != color {
                        moves.push(self.create_move(from, to_sq, None, false, false));
                    }
                    break;
                } else {
                    moves.push(self.create_move(from, to_sq, None, false, false));
                }
                r += dr;
                f += df;
            }
        }
        moves
    }

    fn generate_king_moves(&self, from: Square) -> Vec<Move> {
        let mut moves = Vec::new();
        let deltas = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        let (rank, file) = (from.0, from.1);
        let color = self.current_color();
        let back_rank = if color == Color::White { 0 } else { 7 };

        for (dr, df) in deltas {
            let (nr, nf) = (rank as isize + dr, file as isize + df);
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                let to_sq = Square(nr as usize, nf as usize);
                if let Some((c, _)) = self.squares[to_sq.0][to_sq.1] {
                    if c != color {
                        moves.push(self.create_move(from, to_sq, None, false, false));
                    }
                } else {
                    moves.push(self.create_move(from, to_sq, None, false, false));
                }
            }
        }

        if from == Square(back_rank, 4) {
            if self.castling_rights.contains(&(color, 'K'))
                && self.squares[back_rank][5].is_none()
                && self.squares[back_rank][6].is_none()
                && self.squares[back_rank][7] == Some((color, Piece::Rook))
            {
                let to_sq = Square(back_rank, 6);
                moves.push(self.create_move(from, to_sq, None, true, false));
            }
            if self.castling_rights.contains(&(color, 'Q'))
                 && self.squares[back_rank][1].is_none()
                 && self.squares[back_rank][2].is_none()
                 && self.squares[back_rank][3].is_none()
                 && self.squares[back_rank][0] == Some((color, Piece::Rook))
            {
                let to_sq = Square(back_rank, 2);
                moves.push(self.create_move(from, to_sq, None, true, false));
            }
        }

        moves
    }

    fn find_king(&self, color: Color) -> Option<Square> {
        for r in 0..8 {
            for f in 0..8 {
                if self.squares[r][f] == Some((color, Piece::King)) {
                    return Some(Square(r, f));
                }
            }
        }
        None
    }

    fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        let target_r = square.0 as isize;
        let target_f = square.1 as isize;

        let pawn_dir: isize = if attacker_color == Color::White {
            1
        } else {
            -1
        };
        let pawn_start_r = target_r - pawn_dir;
        if pawn_start_r >= 0 && pawn_start_r < 8 {
            for df in [-1, 1] {
                let pawn_start_f = target_f + df;
                if pawn_start_f >= 0 && pawn_start_f < 8 {
                    if self.squares[pawn_start_r as usize][pawn_start_f as usize]
                        == Some((attacker_color, Piece::Pawn))
                    {
                        return true;
                    }
                }
            }
        }

        let knight_deltas = [
            (2, 1),
            (1, 2),
            (-1, 2),
            (-2, 1),
            (-2, -1),
            (-1, -2),
            (1, -2),
            (2, -1),
        ];
        for (dr, df) in knight_deltas {
            let r = target_r + dr;
            let f = target_f + df;
            if r >= 0 && r < 8 && f >= 0 && f < 8 {
                if self.squares[r as usize][f as usize] == Some((attacker_color, Piece::Knight)) {
                    return true;
                }
            }
        }

        let king_deltas = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        for (dr, df) in king_deltas {
            let r = target_r + dr;
            let f = target_f + df;
            if r >= 0 && r < 8 && f >= 0 && f < 8 {
                if self.squares[r as usize][f as usize] == Some((attacker_color, Piece::King)) {
                    return true;
                }
            }
        }

        let sliding_directions = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];

        for (i, &(dr, df)) in sliding_directions.iter().enumerate() {
            let is_diagonal = i >= 4;
            let mut r = target_r + dr;
            let mut f = target_f + df;

            while r >= 0 && r < 8 && f >= 0 && f < 8 {
                if let Some((piece_color, piece)) = self.squares[r as usize][f as usize] {
                    if piece_color == attacker_color {
                        let can_attack = match piece {
                            Piece::Queen => true,
                            Piece::Rook => !is_diagonal,
                            Piece::Bishop => is_diagonal,
                            _ => false,
                        };
                        if can_attack {
                            return true;
                        }
                    }
                    break;
                }
                r += dr;
                f += df;
            }
        }

        false
    }

    fn is_in_check(&self, color: Color) -> bool {
        if let Some(king_sq) = self.find_king(color) {
            self.is_square_attacked(king_sq, self.opponent_color(color))
        } else {
            false
        }
    }

    pub fn generate_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color();
        let opponent_color = self.opponent_color(current_color);
        let pseudo_moves = self.generate_pseudo_moves();
        let mut legal_moves = Vec::new();

        for m in pseudo_moves {
            if m.is_castling {
                let king_start_sq = m.from;
                let king_mid_sq = Square(m.from.0, (m.from.1 + m.to.1) / 2);
                let king_end_sq = m.to;

                if self.is_square_attacked(king_start_sq, opponent_color)
                    || self.is_square_attacked(king_mid_sq, opponent_color)
                    || self.is_square_attacked(king_end_sq, opponent_color)
                {
                    continue;
                }
            }

            let info = self.make_move(&m);
            if !self.is_in_check(current_color) {
                legal_moves.push(m.clone());
            }
            self.unmake_move(&m, info);
        }
        legal_moves
    }

    pub fn is_checkmate(&mut self) -> bool {
        let color = self.current_color();
        self.is_in_check(color) && self.generate_moves().is_empty()
    }

    pub fn is_stalemate(&mut self) -> bool {
        let color = self.current_color();
        !self.is_in_check(color) && self.generate_moves().is_empty()
    }

    pub fn evaluate(&self) -> i32 {
        let mut score = 0;

        const MATERIAL_MG: [i32; 6] = [82, 337, 365, 477, 1025, 20000];
        const MATERIAL_EG: [i32; 6] = [94, 281, 297, 512, 936, 20000];

        const PST_MG: [[i32; 64]; 6] = [
            [
                0, 0, 0, 0, 0, 0, 0, 0, -35, -1, -20, -23, -15, 24, 38, -22, -26, -4, -4, -10, 3,
                3, 33, -12, -27, -2, -5, 12, 17, 6, 10, -25, -14, 13, 6, 21, 23, 12, 17, -23, -6,
                7, 26, 31, 65, 56, 25, -20, 98, 134, 61, 95, 68, 126, 34, -11, 0, 0, 0, 0, 0, 0, 0,
                0,
            ],
            [
                -105, -21, -58, -33, -17, -28, -19, -23, -29, -53, -12, -3, -1, 18, -14, -19, -23,
                -9, 12, 10, 19, 17, 25, -16, -13, 4, 16, 13, 28, 19, 21, -8, -9, 17, 19, 53, 37,
                69, 18, 22, -47, 60, 37, 65, 84, 129, 73, 44, -73, -41, 72, 36, 23, 62, 7, -17,
                -167, -89, -34, -49, 61, -97, -15, -107,
            ],
            [
                -33, -3, -14, -21, -13, -12, -39, -21, 4, 15, 16, 0, 7, 21, 33, 1, 0, 15, 15, 15,
                14, 27, 18, 10, -6, 13, 13, 26, 34, 12, 10, 4, -4, 5, 19, 50, 37, 37, 7, -2, -16,
                37, 43, 40, 35, 50, 37, -2, -26, 16, -18, -13, 30, 59, 18, -47, -29, 4, -82, -37,
                -25, -42, 7, -8,
            ],
            [
                -19, -13, 1, 17, 16, 7, -37, -26, -44, -16, -20, -9, -1, 11, -6, -71, -45, -25,
                -16, -17, 3, 0, -5, -33, -36, -26, -12, -1, 9, -7, 6, -23, -24, -11, 7, 26, 24, 35,
                -8, -20, -5, 19, 26, 36, 17, 45, 61, 16, 27, 32, 58, 62, 80, 67, 26, 44, 32, 42,
                32, 51, 63, 9, 31, 43,
            ],
            [
                -1, -18, -9, 10, -15, -25, -31, -50, -35, -8, 11, 2, 8, 15, -3, 1, -14, 2, -11, -2,
                -5, 2, 14, 5, -9, -26, -9, -10, -2, -4, 3, -3, -27, -27, -16, -16, -1, 17, -2, 1,
                -13, -17, 7, 8, 29, 56, 47, 57, -24, -39, -5, 1, -16, 57, 28, 54, -28, 0, 29, 12,
                59, 44, 43, 45,
            ],
            [
                -15, 36, 12, -54, 8, -28, 34, 14, 1, 7, -8, -64, -43, -16, 9, 8, -14, -14, -22,
                -46, -44, -30, -15, -27, -49, -1, -27, -39, -46, -44, -33, -51, -17, -20, -12, -27,
                -30, -25, -14, -36, -9, 24, 2, -16, -20, 6, 22, -22, 29, -1, -20, -7, -8, -4, -38,
                -29, -65, 23, 16, -15, -56, -34, 2, 13,
            ],
        ];

        const PST_EG: [[i32; 64]; 6] = [
            [
                0, 0, 0, 0, 0, 0, 0, 0, 13, 8, 8, 10, 13, 0, 2, -7, 4, 7, -6, 1, 0, -5, -1, -8, 13,
                9, -3, -7, -7, -8, 3, -1, 32, 24, 13, 5, -2, 4, 17, 17, 94, 100, 85, 67, 56, 53,
                82, 84, 178, 173, 158, 134, 147, 132, 165, 187, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
            [
                -29, -51, -23, -15, -22, -18, -50, -64, -42, -20, -10, -5, -2, -20, -23, -44, -23,
                -3, -1, 15, 10, -3, -20, -22, -18, -6, 16, 25, 16, 17, 4, -18, -17, 3, 22, 22, 22,
                11, 8, -18, -24, -20, 10, 9, -1, -9, -19, -41, -25, -8, -25, -2, -9, -25, -24, -52,
                -58, -38, -13, -28, -31, -27, -63, -99,
            ],
            [
                -23, -9, -23, -5, -9, -16, -5, -17, -14, -18, -7, -1, 4, -9, -15, -27, -12, -3, 8,
                10, 13, 3, -7, -15, -6, 3, 13, 19, 7, 10, -3, -9, -3, 9, 12, 9, 14, 10, 3, 2, 2,
                -8, 0, -1, -2, 6, 0, 4, -8, -4, 7, -12, -3, -13, -4, -14, -14, -21, -11, -8, -7,
                -9, -17, -24,
            ],
            [
                -9, 2, 3, -1, -5, -13, 4, -20, -6, -6, 0, 2, -9, -9, -11, -3, -4, 0, -5, -1, -7,
                -12, -8, -16, 3, 5, 8, 4, -5, -6, -8, -11, 4, 3, 13, 1, 2, 1, -1, 2, 7, 7, 7, 5, 4,
                -3, -5, -3, 11, 13, 13, 11, -3, 3, 8, 3, 13, 10, 18, 15, 12, 12, 8, 5,
            ],
            [
                -33, -28, -22, -43, -5, -32, -20, -41, -22, -23, -30, -16, -16, -23, -36, -32, -16,
                -27, 15, 6, 9, 17, 10, 5, -18, 28, 19, 47, 31, 34, 39, 23, 3, 22, 24, 45, 57, 40,
                57, 36, -20, 6, 9, 49, 47, 35, 19, 9, -17, 20, 32, 41, 58, 25, 30, 0, -9, 22, 22,
                27, 27, 19, 10, 20,
            ],
            [
                -53, -34, -21, -11, -28, -14, -24, -43, -27, -11, 4, 13, 14, 4, -5, -17, -19, -3,
                11, 21, 23, 16, 7, -9, -18, -4, 21, 24, 27, 23, 9, -11, -8, 22, 24, 27, 26, 33, 26,
                3, 10, 17, 23, 15, 20, 45, 44, 13, -12, 17, 14, 17, 17, 38, 23, 11, -74, -35, -18,
                -18, -11, 15, 4, -17,
            ],
        ];

        fn square_to_index(rank: usize, file: usize) -> usize {
            rank * 8 + file
        }

        fn piece_to_index(piece: Piece) -> usize {
            match piece {
                Piece::Pawn => 0,
                Piece::Knight => 1,
                Piece::Bishop => 2,
                Piece::Rook => 3,
                Piece::Queen => 4,
                Piece::King => 5,
            }
        }

        let mut white_material_mg = 0;
        let mut black_material_mg = 0;
        let mut white_material_eg = 0;
        let mut black_material_eg = 0;
        let mut white_bishop_count = 0;
        let mut black_bishop_count = 0;
        let mut white_pawns_by_file = [0; 8];
        let mut black_pawns_by_file = [0; 8];

        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.squares[rank][file] {
                    let piece_idx = piece_to_index(piece);

                    if color == Color::White {
                        if piece == Piece::Bishop {
                            white_bishop_count += 1;
                        } else if piece == Piece::Pawn {
                            white_pawns_by_file[file] += 1;
                        }
                        white_material_mg += MATERIAL_MG[piece_idx];
                        white_material_eg += MATERIAL_EG[piece_idx];
                    } else {
                        if piece == Piece::Bishop {
                            black_bishop_count += 1;
                        } else if piece == Piece::Pawn {
                            black_pawns_by_file[file] += 1;
                        }
                        black_material_mg += MATERIAL_MG[piece_idx];
                        black_material_eg += MATERIAL_EG[piece_idx];
                    }
                }
            }
        }

        let total_material_mg = white_material_mg + black_material_mg;
        let max_material = 2
            * (MATERIAL_MG[1] * 2
                + MATERIAL_MG[2] * 2
                + MATERIAL_MG[3] * 2
                + MATERIAL_MG[4]
                + MATERIAL_MG[0] * 8);
        let phase = (total_material_mg as f32) / (max_material as f32);
        let phase = phase.min(1.0).max(0.0);

        let mut mg_score = 0;
        let mut eg_score = 0;

        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.squares[rank][file] {
                    let piece_idx = piece_to_index(piece);

                    let sq_idx = if color == Color::White {
                        square_to_index(7 - rank, file)
                    } else {
                        square_to_index(rank, file)
                    };

                    let mg_value = MATERIAL_MG[piece_idx] + PST_MG[piece_idx][sq_idx];
                    let eg_value = MATERIAL_EG[piece_idx] + PST_EG[piece_idx][sq_idx];

                    if color == Color::White {
                        mg_score += mg_value;
                        eg_score += eg_value;
                    } else {
                        mg_score -= mg_value;
                        eg_score -= eg_value;
                    }
                }
            }
        }

        let position_score = (phase * mg_score as f32 + (1.0 - phase) * eg_score as f32) as i32;
        score += position_score;

        if white_bishop_count >= 2 {
            score += 30;
        }
        if black_bishop_count >= 2 {
            score -= 30;
        }

        for file in 0..8 {
            for rank in 0..8 {
                if let Some((color, piece)) = self.squares[rank][file] {
                    if piece == Piece::Rook {
                        let file_pawns = white_pawns_by_file[file] + black_pawns_by_file[file];

                        if file_pawns == 0 {
                            let bonus = 15;
                            if color == Color::White {
                                score += bonus;
                            } else {
                                score -= bonus;
                            }
                        } else if (color == Color::White && black_pawns_by_file[file] == 0)
                            || (color == Color::Black && white_pawns_by_file[file] == 0)
                        {
                            let bonus = 7;
                            if color == Color::White {
                                score += bonus;
                            } else {
                                score -= bonus;
                            }
                        }
                    }
                }
            }
        }

        for file in 0..8 {
            if white_pawns_by_file[file] > 0 {
                let left_file = if file > 0 {
                    white_pawns_by_file[file - 1]
                } else {
                    0
                };
                let right_file = if file < 7 {
                    white_pawns_by_file[file + 1]
                } else {
                    0
                };

                if left_file == 0 && right_file == 0 {
                    score -= 12;
                }
            }

            if black_pawns_by_file[file] > 0 {
                let left_file = if file > 0 {
                    black_pawns_by_file[file - 1]
                } else {
                    0
                };
                let right_file = if file < 7 {
                    black_pawns_by_file[file + 1]
                } else {
                    0
                };

                if left_file == 0 && right_file == 0 {
                    score += 12;
                }
            }

            if white_pawns_by_file[file] > 1 {
                score -= 12 * (white_pawns_by_file[file] - 1);
            }

            if black_pawns_by_file[file] > 1 {
                score += 12 * (black_pawns_by_file[file] - 1);
            }

            for rank in 0..8 {
                if let Some((Color::White, Piece::Pawn)) = self.squares[rank][file] {
                    let mut is_passed = true;

                    for check_rank in 0..rank {
                        for check_file in file.saturating_sub(1)..=(file + 1).min(7) {
                            if let Some((Color::Black, Piece::Pawn)) =
                                self.squares[check_rank][check_file]
                            {
                                is_passed = false;
                                break;
                            }
                        }
                        if !is_passed {
                            break;
                        }
                    }

                    if is_passed {
                        let bonus = 10 + (7 - rank as i32) * 7;
                        score += bonus;
                    }
                } else if let Some((Color::Black, Piece::Pawn)) = self.squares[rank][file] {
                    let mut is_passed = true;

                    for check_rank in (rank + 1)..8 {
                        for check_file in file.saturating_sub(1)..=(file + 1).min(7) {
                            if let Some((Color::White, Piece::Pawn)) =
                                self.squares[check_rank][check_file]
                            {
                                is_passed = false;
                                break;
                            }
                        }
                        if !is_passed {
                            break;
                        }
                    }

                    if is_passed {
                        let bonus = 10 + rank as i32 * 7;
                        score -= bonus;
                    }
                }
            }
        }

        if self.white_to_move {
            score
        } else {
            -score
        }
    }

    pub fn negamax(
        &mut self,
        tt: &mut TranspositionTable,
        depth: u32,
        mut alpha: i32,
        mut beta: i32,
    ) -> i32 {
        let original_alpha = alpha;
        let current_hash = self.hash;

        let mut hash_move: Option<Move> = None;
        if let Some(entry) = tt.probe(current_hash) {
            if entry.depth >= depth {
                match entry.bound_type {
                    BoundType::Exact => return entry.score,
                    BoundType::LowerBound => alpha = alpha.max(entry.score),
                    BoundType::UpperBound => beta = beta.min(entry.score),
                }
                if alpha >= beta {
                    return entry.score;
                }
            }
            hash_move = entry.best_move.clone();
        }

        if depth == 0 {
            return self.quiesce(tt, alpha, beta);
        }

        let mut legal_moves = self.generate_moves();
        legal_moves.sort_by_key(|m| -mvv_lva_score(m, self));

        if legal_moves.is_empty() {
            let current_color = self.current_color();
            return if self.is_in_check(current_color) {
                -(MATE_SCORE - (100 - depth as i32))
            } else {
                0
            };
        }

        if let Some(hm) = &hash_move {
            if let Some(pos) = legal_moves.iter().position(|m| m == hm) {
                legal_moves.swap(0, pos);
            }
        }

        let mut best_score = -MATE_SCORE * 2;
        let mut best_move_found: Option<Move> = None;

        for (i, m) in legal_moves.iter().enumerate() {
            let info = self.make_move(&m);
            let score = if i == 0 {
                -self.negamax(tt, depth - 1, -beta, -alpha)
            } else {
                let mut score = -self.negamax(tt, depth - 1, -alpha - 1, -alpha);
                if score > alpha && score < beta {
                    score = -self.negamax(tt, depth - 1, -beta, -alpha);
                }
                score
            };
            self.unmake_move(&m, info);

            if score > best_score {
                best_score = score;
                best_move_found = Some(m.clone());
            }

            alpha = alpha.max(best_score);

            if alpha >= beta {
                break;
            }
        }

        let bound_type = if best_score <= original_alpha {
            BoundType::UpperBound
        } else if best_score >= beta {
            BoundType::LowerBound
        } else {
            BoundType::Exact
        };

        tt.store(current_hash, depth, best_score, bound_type, best_move_found);

        best_score
    }

    pub fn quiesce(
        &mut self,
        tt: &mut TranspositionTable,
        mut alpha: i32,
        beta: i32,
    ) -> i32 {
        let stand_pat_score = self.evaluate();

        if stand_pat_score >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat_score);

        let mut tactical_moves = self.generate_tactical_moves();
        tactical_moves.sort_by_key(|m| -mvv_lva_score(m, self));

        let mut best_score = stand_pat_score;

        for m in tactical_moves {
            let info = self.make_move(&m);
            let score = -self.quiesce(tt, -beta, -alpha);
            self.unmake_move(&m, info);

            best_score = best_score.max(score);
            alpha = alpha.max(best_score);

            if alpha >= beta {
                break;
            }
        }

        alpha
    }

    pub fn generate_tactical_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color();

        let mut pseudo_tactical_moves = Vec::new();
        for r in 0..8 {
            for f in 0..8 {
                if let Some((c, piece)) = self.squares[r][f] {
                    if c == current_color {
                        let from = Square(r, f);
                        match piece {
                            Piece::Pawn => {
                                self.generate_pawn_tactical_moves(from, &mut pseudo_tactical_moves);
                            }
                            _ => {
                                let piece_moves = self.generate_piece_moves(from, piece);
                                for m in piece_moves {
                                    if m.captured_piece.is_some() || m.is_en_passant {
                                        pseudo_tactical_moves.push(m);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut legal_tactical_moves = Vec::new();
        for m in pseudo_tactical_moves {
            if m.is_castling {
                continue;
            }

            let info = self.make_move(&m);
            if !self.is_in_check(current_color) {
                legal_tactical_moves.push(m.clone());
            }
            self.unmake_move(&m, info);
        }

        legal_tactical_moves
    }

    fn generate_pawn_tactical_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.current_color();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        let r = from.0 as isize;
        let f = from.1 as isize;

        let forward_r = r + dir;

        if forward_r >= 0 && forward_r < 8 {
            let forward_sq = Square(forward_r as usize, f as usize);
            if forward_sq.0 == promotion_rank && self.squares[forward_sq.0][forward_sq.1].is_none()
            {
                for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                    moves.push(self.create_move(from, forward_sq, Some(promo), false, false));
                }
            }
        }

        if forward_r >= 0 && forward_r < 8 {
            for df in [-1, 1] {
                let capture_f = f + df;
                if capture_f >= 0 && capture_f < 8 {
                    let target_sq = Square(forward_r as usize, capture_f as usize);

                    if let Some((target_color, _)) = self.squares[target_sq.0][target_sq.1] {
                        if target_color != color {
                            if target_sq.0 == promotion_rank {
                                for promo in
                                    [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
                                {
                                    moves.push(self.create_move(
                                        from,
                                        target_sq,
                                        Some(promo),
                                        false,
                                        false,
                                    ));
                                }
                            } else {
                                moves.push(self.create_move(from, target_sq, None, false, false));
                            }
                        }
                    } else if Some(target_sq) == self.en_passant_target {
                        moves.push(self.create_move(from, target_sq, None, false, true));
                    }
                }
            }
        }
    }

    pub fn current_color(&self) -> Color {
        if self.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    pub fn opponent_color(&self, color: Color) -> Color {
        match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    pub fn print(&self) {
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let piece_char = match self.squares[rank][file] {
                    Some((Color::White, Piece::Pawn)) => 'P',
                    Some((Color::White, Piece::Knight)) => 'N',
                    Some((Color::White, Piece::Bishop)) => 'B',
                    Some((Color::White, Piece::Rook)) => 'R',
                    Some((Color::White, Piece::Queen)) => 'Q',
                    Some((Color::White, Piece::King)) => 'K',
                    Some((Color::Black, Piece::Pawn)) => 'p',
                    Some((Color::Black, Piece::Knight)) => 'n',
                    Some((Color::Black, Piece::Bishop)) => 'b',
                    Some((Color::Black, Piece::Rook)) => 'r',
                    Some((Color::Black, Piece::Queen)) => 'q',
                    Some((Color::Black, Piece::King)) => 'k',
                    None => ' ',
                };
                print!(" {} |", piece_char);
            }
            println!("\n  +---+---+---+---+---+---+---+---+");
        }
        println!("    a   b   c   d   e   f   g   h");
        println!(
            "Turn: {}",
            if self.white_to_move { "White" } else { "Black" }
        );
        if let Some(ep_target) = self.en_passant_target {
            println!("EP Target: {}", format_square(ep_target));
        }
        println!("Castling: {:?}", self.castling_rights);
        println!("------------------------------------");
    }
}
