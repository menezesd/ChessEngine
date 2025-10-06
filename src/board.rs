use crate::bitboard::*;
use crate::types::*;
use crate::zobrist::*;

// --- Board ---

#[derive(Clone, Debug)]
pub struct Board {
    pub pieces: [[u64; 6]; 2], // [color][piece_type] bitboards
    pub occ: [u64; 2],         // occupancy for each color
    pub occ_all: u64,          // all pieces
    pub white_to_move: bool,
    pub en_passant_target: Option<Square>,
    pub castling_rights: u8,
    pub hash: u64,
}

impl Board {
    // Helper functions for bitboards
    pub fn piece_to_index(p: Piece) -> usize {
        match p {
            Piece::Pawn => 0,
            Piece::Knight => 1,
            Piece::Bishop => 2,
            Piece::Rook => 3,
            Piece::Queen => 4,
            Piece::King => 5,
        }
    }

    pub fn index_to_piece(idx: usize) -> Piece {
        match idx {
            0 => Piece::Pawn,
            1 => Piece::Knight,
            2 => Piece::Bishop,
            3 => Piece::Rook,
            4 => Piece::Queen,
            5 => Piece::King,
            _ => panic!("Invalid piece index"),
        }
    }

    pub fn current_color(&self) -> Color {
        if self.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    // Get the piece and color at a square (0-63)
    pub fn piece_at(&self, sq: usize) -> Option<(Color, Piece)> {
        for color_idx in 0..2 {
            for piece_idx in 0..6 {
                if (self.pieces[color_idx][piece_idx] & (1u64 << sq)) != 0 {
                    let color = if color_idx == 0 { Color::White } else { Color::Black };
                    let piece = Self::index_to_piece(piece_idx);
                    return Some((color, piece));
                }
            }
        }
        None
    }

    pub fn new() -> Self {
        let mut pieces = [[0u64; 6]; 2];

        // White pieces
        pieces[0][0] = 0x000000000000FF00; // Pawns on rank 2
        pieces[0][1] = 0x0000000000000042; // Knights on b1, g1
        pieces[0][2] = 0x0000000000000024; // Bishops on c1, f1
        pieces[0][3] = 0x0000000000000081; // Rooks on a1, h1
        pieces[0][4] = 0x0000000000000008; // Queen on d1
        pieces[0][5] = 0x0000000000000010; // King on e1

        // Black pieces (mirrored)
        pieces[1][0] = 0x00FF000000000000; // Pawns on rank 7
        pieces[1][1] = 0x4200000000000000; // Knights on b8, g8
        pieces[1][2] = 0x2400000000000000; // Bishops on c8, f8
        pieces[1][3] = 0x8100000000000000; // Rooks on a8, h8
        pieces[1][4] = 0x0800000000000000; // Queen on d8
        pieces[1][5] = 0x1000000000000000; // King on e8

        let occ = [
            pieces[0].iter().fold(0, |acc, &bb| acc | bb),
            pieces[1].iter().fold(0, |acc, &bb| acc | bb),
        ];
        let occ_all = occ[0] | occ[1];

        let castling_rights = WHITE_KINGSIDE | WHITE_QUEENSIDE | BLACK_KINGSIDE | BLACK_QUEENSIDE;

        let mut board = Board {
            pieces,
            occ,
            occ_all,
            white_to_move: true,
            en_passant_target: None,
            castling_rights,
            hash: 0,
        };
        board.hash = board.calculate_initial_hash();
        board
    }

    pub fn from_fen(fen: &str) -> Self {
        let mut pieces = [[0u64; 6]; 2];
        let mut castling_rights = 0u8;
        let parts: Vec<&str> = fen.split_whitespace().collect();
        assert!(parts.len() >= 4, "FEN must have at least 4 parts");

        // Parse piece placement
        for (rank_idx, rank_str) in parts[0].split('/').enumerate() {
            let mut file = 0;
            for c in rank_str.chars() {
                if c.is_digit(10) {
                    file += c.to_digit(10).unwrap() as usize;
                } else {
                    let (color_idx, piece_idx) = match c {
                        'P' => (0, 0), // White Pawn
                        'N' => (0, 1), // White Knight
                        'B' => (0, 2), // White Bishop
                        'R' => (0, 3), // White Rook
                        'Q' => (0, 4), // White Queen
                        'K' => (0, 5), // White King
                        'p' => (1, 0), // Black Pawn
                        'n' => (1, 1), // Black Knight
                        'b' => (1, 2), // Black Bishop
                        'r' => (1, 3), // Black Rook
                        'q' => (1, 4), // Black Queen
                        'k' => (1, 5), // Black King
                        _ => panic!("Invalid piece char"),
                    };
                    let sq = (7 - rank_idx) * 8 + file;
                    pieces[color_idx][piece_idx] |= 1u64 << sq;
                    file += 1;
                }
            }
        }

        let white_to_move = match parts[1] {
            "w" => true,
            "b" => false,
            _ => panic!("Invalid color"),
        };

        // Parse castling rights
        for c in parts[2].chars() {
            match c {
                'K' => {
                    castling_rights |= WHITE_KINGSIDE;
                }
                'Q' => {
                    castling_rights |= WHITE_QUEENSIDE;
                }
                'k' => {
                    castling_rights |= BLACK_KINGSIDE;
                }
                'q' => {
                    castling_rights |= BLACK_QUEENSIDE;
                }
                '-' => {}
                _ => panic!("Invalid castle"),
            }
        }

        let en_passant_target = if parts[3] != "-" {
            let chars: Vec<char> = parts[3].chars().collect();
            if chars.len() == 2 {
                Some(Square(rank_to_index(chars[1]), file_to_index(chars[0])))
            } else {
                None
            }
        } else {
            None
        };

        let occ = [
            pieces[0].iter().fold(0, |acc, &bb| acc | bb),
            pieces[1].iter().fold(0, |acc, &bb| acc | bb),
        ];
        let occ_all = occ[0] | occ[1];

        let mut board = Board {
            pieces,
            occ,
            occ_all,
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

        // Pieces
        for c_idx in 0..2 {
            for p_idx in 0..6 {
                let mut bb = self.pieces[c_idx][p_idx];
                while bb != 0 {
                    let sq = bb.trailing_zeros() as usize;
                    bb &= bb - 1;
                    hash ^= zobrist_piece(c_idx, p_idx, sq);
                }
            }
        }

        // Side to move
        if !self.white_to_move {
            hash ^= *ZOBRIST_SIDE_TO_MOVE;
        }

        // Castling rights
        hash ^= zobrist_castling(self.castling_rights);

        // En passant target
        if let Some(ep_square) = self.en_passant_target {
            hash ^= zobrist_en_passant(Some(ep_square.0 * 8 + ep_square.1));
        }

        hash
    }

    // Make/Unmake logic
    pub fn make_move(&mut self, m: &Move) -> UnmakeInfo {
        let previous_hash = self.hash;
        let previous_en_passant_target = self.en_passant_target.clone();
        let previous_castling_rights = self.castling_rights.clone();
        let mut captured_piece_info: Option<(Color, Piece)> = None;

        let color_idx = if self.white_to_move { 0 } else { 1 };
        let opp_color_idx = 1 - color_idx;
        let color = if color_idx == 0 {
            Color::White
        } else {
            Color::Black
        };
        let opp_color = if color_idx == 0 {
            Color::Black
        } else {
            Color::White
        };

        let from_sq = m.from.0 * 8 + m.from.1;
        let to_sq = m.to.0 * 8 + m.to.1;

        // Find moving piece type
        let mut moving_pt = 0;
        for pt in 0..6 {
            if self.pieces[color_idx][pt] & (1u64 << from_sq) != 0 {
                moving_pt = pt;
                break;
            }
        }

        // Handle capture
        let mut captured_sq = None;
        if m.is_en_passant {
            captured_sq = Some(if color_idx == 0 { to_sq - 8 } else { to_sq + 8 });
            captured_piece_info = Some((opp_color, Piece::Pawn));
        } else if !m.is_castling && (self.occ[opp_color_idx] & (1u64 << to_sq)) != 0 {
            captured_sq = Some(to_sq);
            for pt in 0..6 {
                if self.pieces[opp_color_idx][pt] & (1u64 << to_sq) != 0 {
                    captured_piece_info = Some((opp_color, Self::index_to_piece(pt)));
                    break;
                }
            }
        }

        // Update hash: side to move
        self.hash ^= *ZOBRIST_SIDE_TO_MOVE;

        // Update hash: remove captured piece
        if let Some(cap_sq) = captured_sq {
            if let Some((cap_col, cap_piece)) = captured_piece_info {
                let cap_sq_idx = cap_sq;
                let cap_col_idx = if cap_col == Color::White { 0 } else { 1 };
                let cap_pt_idx = Self::piece_to_index(cap_piece);
                self.hash ^= zobrist_piece(cap_col_idx, cap_pt_idx, cap_sq_idx);
            }
        }

        // Update hash: remove moving piece from from
        self.hash ^= zobrist_piece(color_idx, moving_pt, from_sq);

        // Move the piece
        self.pieces[color_idx][moving_pt] ^= (1u64 << from_sq) | (1u64 << to_sq);
        self.occ[color_idx] ^= (1u64 << from_sq) | (1u64 << to_sq);
        self.occ_all ^= (1u64 << from_sq) | (1u64 << to_sq);

        // Remove captured piece
        if let Some(cap_sq) = captured_sq {
            if m.is_en_passant {
                self.pieces[opp_color_idx][0] ^= 1u64 << cap_sq;
            } else {
                let mut cap_pt: usize = 0;
                for pt in 0..6 {
                    if self.pieces[opp_color_idx][pt] & (1u64 << cap_sq) != 0 {
                        cap_pt = pt;
                        break;
                    }
                }
                self.pieces[opp_color_idx][cap_pt] ^= 1u64 << cap_sq;
            }
            self.occ[opp_color_idx] ^= 1u64 << cap_sq;
            self.occ_all ^= 1u64 << cap_sq;
        }

        // Handle promotion
        let final_pt = if let Some(promo) = m.promotion {
            let promo_pt = Self::piece_to_index(promo);
            self.pieces[color_idx][moving_pt] ^= 1u64 << to_sq;
            self.pieces[color_idx][promo_pt] ^= 1u64 << to_sq;
            promo_pt
        } else {
            moving_pt
        };

        // Update hash: add piece to to
        self.hash ^= zobrist_piece(color_idx, final_pt, to_sq);

        // Handle castling
        if m.is_castling {
            let (rook_from_file, rook_to_file) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_from_sq = m.from.0 * 8 + rook_from_file;
            let rook_to_sq = m.from.0 * 8 + rook_to_file;
            self.pieces[color_idx][3] ^= (1u64 << rook_from_sq) | (1u64 << rook_to_sq);
            self.occ[color_idx] ^= (1u64 << rook_from_sq) | (1u64 << rook_to_sq);
            self.occ_all ^= (1u64 << rook_from_sq) | (1u64 << rook_to_sq);
            self.hash ^= zobrist_piece(color_idx, 3, rook_from_sq);
            self.hash ^= zobrist_piece(color_idx, 3, rook_to_sq);
        }

        // Update en passant target
        self.en_passant_target = if moving_pt == 0 && (m.from.0 as i32 - m.to.0 as i32).abs() == 2 {
            let ep_row = (m.from.0 + m.to.0) / 2;
            Some(Square(ep_row, m.from.1))
        } else {
            None
        };

        // Update hash for en passant
        if let Some(old_ep) = previous_en_passant_target {
            self.hash ^= zobrist_en_passant(Some(old_ep.0 * 8 + old_ep.1));
        }
        if let Some(new_ep) = self.en_passant_target {
            self.hash ^= zobrist_en_passant(Some(new_ep.0 * 8 + new_ep.1));
        }

        // Update castling rights
        if moving_pt == 5 {
            // king
            if color == Color::White {
                self.castling_rights &= !(WHITE_KINGSIDE | WHITE_QUEENSIDE);
            } else {
                self.castling_rights &= !(BLACK_KINGSIDE | BLACK_QUEENSIDE);
            }
        }
        if moving_pt == 3 {
            // rook
            let (wq_r, wq_f) = (0, 0);
            let (wk_r, wk_f) = (0, 7);
            let (bq_r, bq_f) = (7, 0);
            let (bk_r, bk_f) = (7, 7);
            if m.from.0 == wq_r && m.from.1 == wq_f {
                self.castling_rights &= !WHITE_QUEENSIDE;
            }
            if m.from.0 == wk_r && m.from.1 == wk_f {
                self.castling_rights &= !WHITE_KINGSIDE;
            }
            if m.from.0 == bq_r && m.from.1 == bq_f {
                self.castling_rights &= !BLACK_QUEENSIDE;
            }
            if m.from.0 == bk_r && m.from.1 == bk_f {
                self.castling_rights &= !BLACK_KINGSIDE;
            }
        }

        if let Some((cap_col, Piece::Rook)) = captured_piece_info {
            let (oq_r, oq_f) = if cap_col == Color::White {
                (0, 0)
            } else {
                (7, 0)
            };
            let (ok_r, ok_f) = if cap_col == Color::White {
                (0, 7)
            } else {
                (7, 7)
            };
            let cap_sq_r = m.to.0;
            let cap_sq_f = m.to.1;
            if cap_sq_r == oq_r && cap_sq_f == oq_f {
                if cap_col == Color::White {
                    self.castling_rights &= !WHITE_QUEENSIDE;
                } else {
                    self.castling_rights &= !BLACK_QUEENSIDE;
                }
            }
            if cap_sq_r == ok_r && cap_sq_f == ok_f {
                if cap_col == Color::White {
                    self.castling_rights &= !WHITE_KINGSIDE;
                } else {
                    self.castling_rights &= !BLACK_KINGSIDE;
                }
            }
        }

        // Update hash for castling rights changes
        let old_castling_hash = zobrist_castling(previous_castling_rights);
        let new_castling_hash = zobrist_castling(self.castling_rights);
        self.hash ^= old_castling_hash ^ new_castling_hash;

        // Flip side to move
        self.white_to_move = !self.white_to_move;

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

        let color_idx = if self.white_to_move { 0 } else { 1 };
        let _opp_color_idx = 1 - color_idx;

        let from_sq = m.from.0 * 8 + m.from.1;
        let to_sq = m.to.0 * 8 + m.to.1;

        let mut moved_pt = 0;
        for pt in 0..6 {
            if self.pieces[color_idx][pt] & (1u64 << to_sq) != 0 {
                moved_pt = pt;
                break;
            }
        }

        let actual_moved_pt = if m.promotion.is_some() { 0 } else { moved_pt };

        self.pieces[color_idx][moved_pt] ^= 1u64 << to_sq;
        self.pieces[color_idx][actual_moved_pt] ^= 1u64 << from_sq;
        self.occ[color_idx] ^= (1u64 << from_sq) | (1u64 << to_sq);
        self.occ_all ^= (1u64 << from_sq) | (1u64 << to_sq);

        if let Some((cap_col, cap_piece)) = info.captured_piece_info {
            let cap_color_idx = if cap_col == Color::White { 0 } else { 1 };
            let cap_pt = Self::piece_to_index(cap_piece);
            let cap_sq = if m.is_en_passant {
                if color_idx == 0 {
                    to_sq - 8
                } else {
                    to_sq + 8
                }
            } else {
                to_sq
            };
            self.pieces[cap_color_idx][cap_pt] ^= 1u64 << cap_sq;
            self.occ[cap_color_idx] ^= 1u64 << cap_sq;
            self.occ_all ^= 1u64 << cap_sq;
        }

        if m.is_castling {
            let (rook_from_file, rook_to_file) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_from_sq = m.from.0 * 8 + rook_from_file;
            let rook_to_sq = m.from.0 * 8 + rook_to_file;
            self.pieces[color_idx][3] ^= (1u64 << rook_from_sq) | (1u64 << rook_to_sq);
            self.occ[color_idx] ^= (1u64 << rook_from_sq) | (1u64 << rook_to_sq);
            self.occ_all ^= (1u64 << rook_from_sq) | (1u64 << rook_to_sq);
        }
    }

    // Null-move: pass the turn without making a move (used for null-move pruning)
    pub fn do_null(&mut self) -> crate::types::NullInfo {
        let previous_en_passant_target = self.en_passant_target.clone();
        let previous_castling_rights = self.castling_rights.clone();
        let previous_hash = self.hash;

        // flip side to move and update zobrist
        self.hash ^= *ZOBRIST_SIDE_TO_MOVE;
        self.white_to_move = !self.white_to_move;

        // remove en-passant target
        self.en_passant_target = None;

        crate::types::NullInfo {
            previous_en_passant_target,
            previous_castling_rights,
            previous_hash,
        }
    }

    pub fn undo_null(&mut self, info: crate::types::NullInfo) {
        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash;
    }

    pub fn can_try_null_move(&self) -> bool {
        // don't do null move if in check or if there is no material to allow null
        if self.is_in_check(self.current_color()) {
            return false;
        }
        // Avoid null move in potential zugzwang-ish endgames: if only kings and pawns remain, be conservative
        let occ_all = self.occ_all;
        let pawns_and_kings =
            (self.pieces[0][0] | self.pieces[1][0]) | (self.pieces[0][5] | self.pieces[1][5]);
        let non_pawn_non_king = occ_all & !pawns_and_kings;
        if non_pawn_non_king == 0 {
            // only pawns and kings present -> be conservative: if pawn count is small, avoid null move (zugzwang risk)
            let pawn_count =
                (self.pieces[0][0].count_ones() + self.pieces[1][0].count_ones()) as usize;
            if pawn_count <= 6 {
                return false;
            }
        }
        true
    }

    // Move generation
    pub fn generate_pseudo_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        let color_idx = if self.white_to_move { 0 } else { 1 };

        for pt in 0..6 {
            let mut bb = self.pieces[color_idx][pt];
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                let from = Square(sq / 8, sq % 8);
                moves.extend(self.generate_piece_moves(from, Self::index_to_piece(pt)));
                bb &= bb - 1;
            }
        }
        moves
    }

    pub fn generate_piece_moves(&self, from: Square, piece: Piece) -> Vec<Move> {
        match piece {
            Piece::Pawn => self.generate_pawn_moves(from),
            _ => {
                let sq = from.0 * 8 + from.1;
                let color_idx = if self.white_to_move { 0 } else { 1 };
                let opp_color_idx = 1 - color_idx;
                let mut moves = Vec::new();

                let attacks = match piece {
                    Piece::Knight => KNIGHT_ATTACKS[sq],
                    Piece::Bishop => bishop_attacks(from, self.occ_all),
                    Piece::Rook => rook_attacks(from, self.occ_all),
                    Piece::Queen => queen_attacks(from, self.occ_all),
                    Piece::King => KING_ATTACKS[sq],
                    _ => 0,
                };

                let quiets = attacks & !self.occ_all;
                let captures = attacks & self.occ[opp_color_idx];

                let mut bb = quiets;
                while bb != 0 {
                    let to_sq = bb.trailing_zeros() as usize;
                    let to = Square(to_sq / 8, to_sq % 8);
                    moves.push(self.create_move(from, to, None, false, false));
                    bb &= bb - 1;
                }

                let mut bb = captures;
                while bb != 0 {
                    let to_sq = bb.trailing_zeros() as usize;
                    let to = Square(to_sq / 8, to_sq % 8);
                    moves.push(self.create_move(from, to, None, false, false));
                    bb &= bb - 1;
                }

                if piece == Piece::King {
                    moves.extend(self.generate_castling_moves(from));
                }

                moves
            }
        }
    }

    pub fn create_move(
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
            let to_sq = to.0 * 8 + to.1;
            let opp_color_idx = if self.white_to_move { 1 } else { 0 };
            if self.occ[opp_color_idx] & (1u64 << to_sq) != 0 {
                let mut cap = None;
                for pt in 0..6 {
                    if self.pieces[opp_color_idx][pt] & (1u64 << to_sq) != 0 {
                        cap = Some(Self::index_to_piece(pt));
                        break;
                    }
                }
                cap
            } else {
                None
            }
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

    pub fn generate_pawn_moves(&self, from: Square) -> Vec<Move> {
        let color_idx = if self.white_to_move { 0 } else { 1 };
        let opp_color_idx = 1 - color_idx;
        let mut moves = Vec::new();
        let dir: isize = if color_idx == 0 { 1 } else { -1 };
        let start_rank = if color_idx == 0 { 1 } else { 6 };
        let promotion_rank = if color_idx == 0 { 7 } else { 0 };

        let r = from.0 as isize;
        let f = from.1 as isize;

        let forward_r = r + dir;
        if forward_r >= 0 && forward_r < 8 {
            let forward_sq = Square(forward_r as usize, f as usize);
            let forward_sq_bit = 1u64 << (forward_sq.0 * 8 + forward_sq.1);
            if self.occ_all & forward_sq_bit == 0 {
                if forward_sq.0 == promotion_rank {
                    for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                        moves.push(self.create_move(from, forward_sq, Some(promo), false, false));
                    }
                } else {
                    moves.push(self.create_move(from, forward_sq, None, false, false));
                    if r == start_rank as isize {
                        let double_forward_r = r + 2 * dir;
                        let double_forward_sq = Square(double_forward_r as usize, f as usize);
                        let double_bit = 1u64 << (double_forward_sq.0 * 8 + double_forward_sq.1);
                        if self.occ_all & double_bit == 0 {
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
                    let target_bit = 1u64 << (target_sq.0 * 8 + target_sq.1);
                    if self.occ[opp_color_idx] & target_bit != 0 {
                        if target_sq.0 == promotion_rank {
                            for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
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
                    } else if Some(target_sq) == self.en_passant_target {
                        moves.push(self.create_move(from, target_sq, None, false, true));
                    }
                }
            }
        }

        moves
    }

    pub fn generate_castling_moves(&self, from: Square) -> Vec<Move> {
        let mut moves = Vec::new();
        let color = self.current_color();
        let color_idx = color as usize;
        let king_start = if color == Color::White {
            Square(0, 4)
        } else {
            Square(7, 4)
        };
        if from != king_start {
            return moves;
        }

        if (self.castling_rights
            & if color == Color::White {
                WHITE_KINGSIDE
            } else {
                BLACK_KINGSIDE
            })
            != 0
        {
            let rook_sq_bit = 1u64 << (from.0 * 8 + 7);
            let king_to = if color == Color::White {
                Square(0, 6)
            } else {
                Square(7, 6)
            };
            let empty1 = 1u64 << (from.0 * 8 + 5);
            let empty2 = 1u64 << (from.0 * 8 + 6);
            if self.occ_all & empty1 == 0
                && self.occ_all & empty2 == 0
                && self.pieces[color_idx][3] & rook_sq_bit != 0
            {
                moves.push(self.create_move(from, king_to, None, true, false));
            }
        }

        if (self.castling_rights
            & if color == Color::White {
                WHITE_QUEENSIDE
            } else {
                BLACK_QUEENSIDE
            })
            != 0
        {
            let rook_sq_bit = 1u64 << (from.0 * 8 + 0);
            let king_to = if color == Color::White {
                Square(0, 2)
            } else {
                Square(7, 2)
            };
            let empty1 = 1u64 << (from.0 * 8 + 1);
            let empty2 = 1u64 << (from.0 * 8 + 2);
            let empty3 = 1u64 << (from.0 * 8 + 3);
            if self.occ_all & empty1 == 0
                && self.occ_all & empty2 == 0
                && self.occ_all & empty3 == 0
                && self.pieces[color_idx][3] & rook_sq_bit != 0
            {
                moves.push(self.create_move(from, king_to, None, true, false));
            }
        }

        moves
    }

    pub fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        let sq = square.0 * 8 + square.1;
        let attacker_idx = if attacker_color == Color::White { 0 } else { 1 };

        let dir = if attacker_color == Color::White {
            1
        } else {
            -1
        };
        let pawn_r = square.0 as isize - dir;
        if pawn_r >= 0 && pawn_r < 8 {
            for &df in &[-1, 1] {
                let pawn_f = square.1 as isize + df;
                if pawn_f >= 0 && pawn_f < 8 {
                    let pawn_sq = (pawn_r as usize) * 8 + pawn_f as usize;
                    if self.pieces[attacker_idx][0] & (1u64 << pawn_sq) != 0 {
                        return true;
                    }
                }
            }
        }

        if KNIGHT_ATTACKS[sq] & self.pieces[attacker_idx][1] != 0 {
            return true;
        }

        if bishop_attacks(square, self.occ_all) & self.pieces[attacker_idx][2] != 0 {
            return true;
        }

        if rook_attacks(square, self.occ_all) & self.pieces[attacker_idx][3] != 0 {
            return true;
        }

        if queen_attacks(square, self.occ_all) & self.pieces[attacker_idx][4] != 0 {
            return true;
        }

        if KING_ATTACKS[sq] & self.pieces[attacker_idx][5] != 0 {
            return true;
        }

        false
    }

    pub fn is_in_check(&self, color: Color) -> bool {
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

    pub fn find_king(&self, color: Color) -> Option<Square> {
        let color_idx = if color == Color::White { 0 } else { 1 };
        let king_bb = self.pieces[color_idx][5];
        if king_bb != 0 {
            let sq = king_bb.trailing_zeros() as usize;
            Some(Square(sq / 8, sq % 8))
        } else {
            None
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
                let sq = rank * 8 + file;
                let piece_char = match self.piece_at(sq) {
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
