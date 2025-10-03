use std::collections::HashSet;
use once_cell::sync::Lazy;
use rand::prelude::*;

use crate::uci::format_square;
use crate::TranspositionTable;

// --- Zobrist Hashing ---
struct ZobristKeys {
    piece_keys: [[[u64; 64]; 2]; 6],
    black_to_move_key: u64,
    castling_keys: [[u64; 2]; 2],
    en_passant_keys: [u64; 8],
}

impl ZobristKeys {
    fn new() -> Self {
        let mut rng = StdRng::seed_from_u64(1234567890_u64);
        let mut piece_keys = [[[0; 64]; 2]; 6];
        let mut castling_keys = [[0; 2]; 2];
        let mut en_passant_keys = [0; 8];

        for p_idx in 0..6 {
            for c_idx in 0..2 {
                for sq_idx in 0..64 {
                    piece_keys[p_idx][c_idx][sq_idx] = rng.gen();
                }
            }
        }
        let black_to_move_key = rng.gen();
        for c_idx in 0..2 { for side_idx in 0..2 { castling_keys[c_idx][side_idx] = rng.gen(); } }
        for f_idx in 0..8 { en_passant_keys[f_idx] = rng.gen(); }
        ZobristKeys { piece_keys, black_to_move_key, castling_keys, en_passant_keys }
    }
}

static ZOBRIST: Lazy<ZobristKeys> = Lazy::new(ZobristKeys::new);

// Helper to map Piece enum to index
pub(crate) fn piece_to_zobrist_index(piece: Piece) -> usize {
    match piece {
        Piece::Pawn => 0,
        Piece::Knight => 1,
        Piece::Bishop => 2,
        Piece::Rook => 3,
        Piece::Queen => 4,
        Piece::King => 5,
    }
}

// Helper to map Color enum to index
pub(crate) fn color_to_zobrist_index(color: Color) -> usize {
    match color { Color::White => 0, Color::Black => 1 }
}

// Helper to map Square to index (0-63)
pub(crate) fn square_to_zobrist_index(sq: Square) -> usize { sq.0 * 8 + sq.1 }

// --- Enums and Structs ---
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Piece { Pawn, Knight, Bishop, Rook, Queen, King }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Color { White, Black }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Square(pub usize, pub usize); // (rank, file)

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Move {
    pub(crate) from: Square,
    pub(crate) to: Square,
    pub(crate) is_castling: bool,
    pub(crate) is_en_passant: bool,
    pub(crate) promotion: Option<Piece>,
    pub(crate) captured_piece: Option<Piece>,
}

#[derive(Clone, Debug)]
pub(crate) struct UnmakeInfo {
    captured_piece_info: Option<(Color, Piece)>,
    previous_en_passant_target: Option<Square>,
    previous_castling_rights: HashSet<(Color, char)>,
    previous_hash: u64,
    previous_halfmove_clock: u32,
}

pub(crate) fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 300,
        Piece::Bishop => 300,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 10000,
    }
}

pub(crate) fn mvv_lva_score(m: &Move, board: &Board) -> i32 {
    if let Some(victim) = m.captured_piece {
        let attacker = board.piece_at(m.from).unwrap().1;
        let victim_value = piece_value(victim);
        let attacker_value = piece_value(attacker);
        victim_value * 10 - attacker_value
    } else { 0 }
}

#[derive(Clone, Debug)]
pub(crate) struct Board {
    // Bitboard representation - piece-centric
    pub(crate) white_pawns: u64,
    pub(crate) white_knights: u64,
    pub(crate) white_bishops: u64,
    pub(crate) white_rooks: u64,
    pub(crate) white_queens: u64,
    pub(crate) white_king: u64,
    pub(crate) black_pawns: u64,
    pub(crate) black_knights: u64,
    pub(crate) black_bishops: u64,
    pub(crate) black_rooks: u64,
    pub(crate) black_queens: u64,
    pub(crate) black_king: u64,
    
    pub(crate) white_to_move: bool,
    pub(crate) en_passant_target: Option<Square>,
    pub(crate) castling_rights: HashSet<(Color, char)>,
    pub(crate) hash: u64,
    pub(crate) halfmove_clock: u32,
    pub(crate) position_history: Vec<u64>,
}

impl Board {
    pub(crate) fn new() -> Self {
        // Initialize bitboards for starting position
        let white_pawns = 0xFF00u64; // Rank 2
        let black_pawns = 0xFF000000000000u64; // Rank 7
        let white_rooks = 0x81u64; // a1, h1
        let black_rooks = 0x8100000000000000u64; // a8, h8
        let white_knights = 0x42u64; // b1, g1
        let black_knights = 0x4200000000000000u64; // b8, g8
        let white_bishops = 0x24u64; // c1, f1
        let black_bishops = 0x2400000000000000u64; // c8, f8
        let white_queens = 0x08u64; // d1
        let black_queens = 0x0800000000000000u64; // d8  
        let white_king = 0x10u64; // e1
        let black_king = 0x1000000000000000u64; // e8
        
        let mut castling_rights = HashSet::new();
        castling_rights.insert((Color::White, 'K'));
        castling_rights.insert((Color::White, 'Q'));
        castling_rights.insert((Color::Black, 'K'));
        castling_rights.insert((Color::Black, 'Q'));
        
        let mut board = Board { 
            white_pawns, white_knights, white_bishops, white_rooks, white_queens, white_king,
            black_pawns, black_knights, black_bishops, black_rooks, black_queens, black_king,
            white_to_move: true, en_passant_target: None, castling_rights, hash: 0, halfmove_clock: 0, position_history: Vec::new() 
        };
        board.hash = board.calculate_initial_hash();
        board.position_history.push(board.hash);
        board
    }
    
    // Helper methods to query pieces at squares
    pub(crate) fn piece_at(&self, sq: Square) -> Option<(Color, Piece)> {
        let bit = 1u64 << (sq.0 * 8 + sq.1);
        
        if (self.white_pawns & bit) != 0 { return Some((Color::White, Piece::Pawn)); }
        if (self.white_knights & bit) != 0 { return Some((Color::White, Piece::Knight)); }
        if (self.white_bishops & bit) != 0 { return Some((Color::White, Piece::Bishop)); }
        if (self.white_rooks & bit) != 0 { return Some((Color::White, Piece::Rook)); }
        if (self.white_queens & bit) != 0 { return Some((Color::White, Piece::Queen)); }
        if (self.white_king & bit) != 0 { return Some((Color::White, Piece::King)); }
        
        if (self.black_pawns & bit) != 0 { return Some((Color::Black, Piece::Pawn)); }
        if (self.black_knights & bit) != 0 { return Some((Color::Black, Piece::Knight)); }
        if (self.black_bishops & bit) != 0 { return Some((Color::Black, Piece::Bishop)); }
        if (self.black_rooks & bit) != 0 { return Some((Color::Black, Piece::Rook)); }
        if (self.black_queens & bit) != 0 { return Some((Color::Black, Piece::Queen)); }
        if (self.black_king & bit) != 0 { return Some((Color::Black, Piece::King)); }
        
        None
    }
    
    pub(crate) fn all_pieces(&self) -> u64 {
        self.white_pawns | self.white_knights | self.white_bishops | self.white_rooks | self.white_queens | self.white_king |
        self.black_pawns | self.black_knights | self.black_bishops | self.black_rooks | self.black_queens | self.black_king
    }
    
    pub(crate) fn white_pieces(&self) -> u64 {
        self.white_pawns | self.white_knights | self.white_bishops | self.white_rooks | self.white_queens | self.white_king
    }
    
    pub(crate) fn black_pieces(&self) -> u64 {
        self.black_pawns | self.black_knights | self.black_bishops | self.black_rooks | self.black_queens | self.black_king
    }
    
    pub(crate) fn pieces_of_color(&self, color: Color) -> u64 {
        match color {
            Color::White => self.white_pieces(),
            Color::Black => self.black_pieces(),
        }
    }
    
    pub(crate) fn set_piece_at(&mut self, sq: Square, color: Color, piece: Piece) {
        let bit = 1u64 << (sq.0 * 8 + sq.1);
        self.clear_square(sq); // Remove any existing piece
        
        match (color, piece) {
            (Color::White, Piece::Pawn) => self.white_pawns |= bit,
            (Color::White, Piece::Knight) => self.white_knights |= bit,
            (Color::White, Piece::Bishop) => self.white_bishops |= bit,
            (Color::White, Piece::Rook) => self.white_rooks |= bit,
            (Color::White, Piece::Queen) => self.white_queens |= bit,
            (Color::White, Piece::King) => self.white_king |= bit,
            (Color::Black, Piece::Pawn) => self.black_pawns |= bit,
            (Color::Black, Piece::Knight) => self.black_knights |= bit,
            (Color::Black, Piece::Bishop) => self.black_bishops |= bit,
            (Color::Black, Piece::Rook) => self.black_rooks |= bit,
            (Color::Black, Piece::Queen) => self.black_queens |= bit,
            (Color::Black, Piece::King) => self.black_king |= bit,
        }
    }
    
    pub(crate) fn clear_square(&mut self, sq: Square) {
        let bit = !(1u64 << (sq.0 * 8 + sq.1));
        self.white_pawns &= bit;
        self.white_knights &= bit;
        self.white_bishops &= bit;
        self.white_rooks &= bit;
        self.white_queens &= bit;
        self.white_king &= bit;
        self.black_pawns &= bit;
        self.black_knights &= bit;
        self.black_bishops &= bit;
        self.black_rooks &= bit;
        self.black_queens &= bit;
        self.black_king &= bit;
    }

    pub(crate) fn from_fen(fen: &str) -> Self {
        // Initialize empty bitboards
        let mut board = Board {
            white_pawns: 0, white_knights: 0, white_bishops: 0, white_rooks: 0, white_queens: 0, white_king: 0,
            black_pawns: 0, black_knights: 0, black_bishops: 0, black_rooks: 0, black_queens: 0, black_king: 0,
            white_to_move: true, en_passant_target: None, castling_rights: HashSet::new(), hash: 0, halfmove_clock: 0, position_history: Vec::new()
        };
        
        let parts: Vec<&str> = fen.split_whitespace().collect();
        assert!(parts.len() >= 4, "FEN must have at least 4 parts");
        for (rank_idx, rank_str) in parts[0].split('/').enumerate() {
            let mut file = 0;
            for c in rank_str.chars() {
                if c.is_digit(10) { file += c.to_digit(10).unwrap() as usize; }
                else {
                    let (color, piece) = match c {
                        'P' => (Color::White, Piece::Pawn), 'N' => (Color::White, Piece::Knight), 'B' => (Color::White, Piece::Bishop), 'R' => (Color::White, Piece::Rook), 'Q' => (Color::White, Piece::Queen), 'K' => (Color::White, Piece::King),
                        'p' => (Color::Black, Piece::Pawn), 'n' => (Color::Black, Piece::Knight), 'b' => (Color::Black, Piece::Bishop), 'r' => (Color::Black, Piece::Rook), 'q' => (Color::Black, Piece::Queen), 'k' => (Color::Black, Piece::King),
                        _ => panic!("Invalid piece char"), };
                    board.set_piece_at(Square(7 - rank_idx, file), color, piece);
                    file += 1;
                }
            }
        }
        board.white_to_move = match parts[1] { "w" => true, "b" => false, _ => panic!("Invalid color") };
        for c in parts[2].chars() {
            match c { 'K' => { board.castling_rights.insert((Color::White, 'K')); }, 'Q' => { board.castling_rights.insert((Color::White, 'Q')); }, 'k' => { board.castling_rights.insert((Color::Black, 'K')); }, 'q' => { board.castling_rights.insert((Color::Black, 'Q')); }, '-' => {}, _ => panic!("Invalid castle"), }
        }
        board.en_passant_target = if parts[3] != "-" { let chars: Vec<char> = parts[3].chars().collect(); if chars.len() == 2 { Some(Square(rank_to_index(chars[1]), file_to_index(chars[0]))) } else { None } } else { None };
        board.halfmove_clock = parts.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        board.hash = board.calculate_initial_hash();
        board.position_history.push(board.hash);
        board
    }

    // Calculate Zobrist hash from scratch
    fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;
        
        // Hash all pieces from bitboards
        let pieces = [
            (self.white_pawns, Color::White, Piece::Pawn),
            (self.white_knights, Color::White, Piece::Knight),
            (self.white_bishops, Color::White, Piece::Bishop),
            (self.white_rooks, Color::White, Piece::Rook),
            (self.white_queens, Color::White, Piece::Queen),
            (self.white_king, Color::White, Piece::King),
            (self.black_pawns, Color::Black, Piece::Pawn),
            (self.black_knights, Color::Black, Piece::Knight),
            (self.black_bishops, Color::Black, Piece::Bishop),
            (self.black_rooks, Color::Black, Piece::Rook),
            (self.black_queens, Color::Black, Piece::Queen),
            (self.black_king, Color::Black, Piece::King),
        ];
        
        for (bb, color, piece) in pieces {
            let mut pieces_bb = bb;
            while pieces_bb != 0 {
                let sq_idx = pieces_bb.trailing_zeros() as usize;
                pieces_bb &= pieces_bb - 1; // Clear the lowest set bit
                let sq = Square(sq_idx / 8, sq_idx % 8);
                let sq_zobrist_idx = square_to_zobrist_index(sq);
                let p_idx = piece_to_zobrist_index(piece);
                let c_idx = color_to_zobrist_index(color);
                hash ^= ZOBRIST.piece_keys[p_idx][c_idx][sq_zobrist_idx];
            }
        }
        
        if !self.white_to_move { hash ^= ZOBRIST.black_to_move_key; }
        if self.castling_rights.contains(&(Color::White, 'K')) { hash ^= ZOBRIST.castling_keys[0][0]; }
        if self.castling_rights.contains(&(Color::White, 'Q')) { hash ^= ZOBRIST.castling_keys[0][1]; }
        if self.castling_rights.contains(&(Color::Black, 'K')) { hash ^= ZOBRIST.castling_keys[1][0]; }
        if self.castling_rights.contains(&(Color::Black, 'Q')) { hash ^= ZOBRIST.castling_keys[1][1]; }
        if let Some(ep_square) = self.en_passant_target { hash ^= ZOBRIST.en_passant_keys[ep_square.1]; }
        hash
    }

    // --- Make/Unmake Logic ---
    pub(crate) fn make_move(&mut self, m: &Move) -> UnmakeInfo {
        let mut current_hash = self.hash;
        let previous_hash = self.hash;
        let color = self.current_color();
        let previous_en_passant_target = self.en_passant_target;
        let previous_castling_rights = self.castling_rights.clone();
        let previous_halfmove_clock = self.halfmove_clock;
        current_hash ^= ZOBRIST.black_to_move_key;
        if let Some(old_ep) = self.en_passant_target { current_hash ^= ZOBRIST.en_passant_keys[old_ep.1]; }
        let mut captured_piece_info: Option<(Color, Piece)> = None;
        if m.is_en_passant {
            let capture_row = if color == Color::White { m.to.0 - 1 } else { m.to.0 + 1 };
            let capture_sq = Square(capture_row, m.to.1);
            let cap_idx = square_to_zobrist_index(capture_sq);
            captured_piece_info = self.piece_at(capture_sq);
            self.clear_square(capture_sq);
            if let Some((cap_col, cap_piece)) = captured_piece_info { current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)][color_to_zobrist_index(cap_col)][cap_idx]; }
        } else if !m.is_castling {
            captured_piece_info = self.piece_at(m.to);
            if captured_piece_info.is_some() {
                let cap_idx = square_to_zobrist_index(m.to);
                if let Some((cap_col, cap_piece)) = captured_piece_info { current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)][color_to_zobrist_index(cap_col)][cap_idx]; }
            }
        }
        let moving_piece_info = self.piece_at(m.from).expect("make_move 'from' empty");
        let (moving_color, moving_piece) = moving_piece_info;
        let from_sq_idx = square_to_zobrist_index(m.from);
        let to_sq_idx = square_to_zobrist_index(m.to);
        current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(moving_piece)][color_to_zobrist_index(moving_color)][from_sq_idx];
        self.clear_square(m.from);
        if m.is_castling {
            self.set_piece_at(m.to, color, Piece::King);
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::King)][color_to_zobrist_index(color)][to_sq_idx];
            let (rook_from_f, rook_to_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_from_sq = Square(m.to.0, rook_from_f);
            let rook_to_sq = Square(m.to.0, rook_to_f);
            let rook_info = self.piece_at(rook_from_sq).expect("Castling without rook");
            self.clear_square(rook_from_sq);
            self.set_piece_at(rook_to_sq, rook_info.0, rook_info.1);
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)][color_to_zobrist_index(color)][square_to_zobrist_index(rook_from_sq)];
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)][color_to_zobrist_index(color)][square_to_zobrist_index(rook_to_sq)];
        } else {
            let piece_to_place = if let Some(promoted_piece) = m.promotion { (color, promoted_piece) } else { moving_piece_info };
            self.set_piece_at(m.to, piece_to_place.0, piece_to_place.1);
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(piece_to_place.1)][color_to_zobrist_index(piece_to_place.0)][to_sq_idx];
        }
        self.en_passant_target = None;
        if moving_piece == Piece::Pawn && (m.from.0 as isize - m.to.0 as isize).abs() == 2 {
            let ep_row = (m.from.0 + m.to.0) / 2; let ep_sq = Square(ep_row, m.from.1); self.en_passant_target = Some(ep_sq); current_hash ^= ZOBRIST.en_passant_keys[ep_sq.1];
        }
        if moving_piece == Piece::Pawn || captured_piece_info.is_some() { self.halfmove_clock = 0; } else { self.halfmove_clock = self.halfmove_clock.saturating_add(1); }
        let mut new_castling_rights = self.castling_rights.clone(); let mut castle_hash_diff: u64 = 0;
        if moving_piece == Piece::King {
            if self.castling_rights.contains(&(color, 'K')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0]; new_castling_rights.remove(&(color, 'K')); }
            if self.castling_rights.contains(&(color, 'Q')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1]; new_castling_rights.remove(&(color, 'Q')); }
        } else if moving_piece == Piece::Rook {
            let start_rank = if color == Color::White { 0 } else { 7 };
            if m.from == Square(start_rank, 0) && self.castling_rights.contains(&(color, 'Q')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1]; new_castling_rights.remove(&(color, 'Q')); }
            else if m.from == Square(start_rank, 7) && self.castling_rights.contains(&(color, 'K')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0]; new_castling_rights.remove(&(color, 'K')); }
        }
        if let Some((captured_color, captured_piece)) = captured_piece_info { if captured_piece == Piece::Rook { let start_rank = if captured_color == Color::White { 0 } else { 7 }; if m.to == Square(start_rank, 0) && self.castling_rights.contains(&(captured_color, 'Q')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][1]; new_castling_rights.remove(&(captured_color, 'Q')); } else if m.to == Square(start_rank, 7) && self.castling_rights.contains(&(captured_color, 'K')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][0]; new_castling_rights.remove(&(captured_color, 'K')); } } }
        self.castling_rights = new_castling_rights; current_hash ^= castle_hash_diff;
        self.white_to_move = !self.white_to_move;
        self.hash = current_hash; self.position_history.push(self.hash);
        UnmakeInfo { captured_piece_info, previous_en_passant_target, previous_castling_rights, previous_hash, previous_halfmove_clock }
    }

    pub(crate) fn unmake_move(&mut self, m: &Move, info: UnmakeInfo) {
        let _ = self.position_history.pop();
        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash;
        self.halfmove_clock = info.previous_halfmove_clock;
        let color = self.current_color();
        let piece_that_moved = if m.promotion.is_some() { (color, Piece::Pawn) } else if m.is_castling { (color, Piece::King) } else { self.piece_at(m.to).expect("Unmake move: 'to' square empty?") };
        if m.is_castling {
            self.set_piece_at(m.from, piece_that_moved.0, piece_that_moved.1); self.clear_square(m.to);
            let (rook_orig_f, rook_moved_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_info = self.piece_at(Square(m.to.0, rook_moved_f)).expect("Unmake castling: rook missing");
            self.clear_square(Square(m.to.0, rook_moved_f)); self.set_piece_at(Square(m.to.0, rook_orig_f), rook_info.0, rook_info.1);
        } else {
            self.set_piece_at(m.from, piece_that_moved.0, piece_that_moved.1);
            if m.is_en_passant {
                self.clear_square(m.to); let capture_row = if color == Color::White { m.to.0 - 1 } else { m.to.0 + 1 }; 
                if let Some((cap_color, cap_piece)) = info.captured_piece_info { self.set_piece_at(Square(capture_row, m.to.1), cap_color, cap_piece); }
            } else { 
                if let Some((cap_color, cap_piece)) = info.captured_piece_info { self.set_piece_at(m.to, cap_color, cap_piece); } else { self.clear_square(m.to); }
            }
        }
    }

    pub(crate) fn make_null_move(&mut self) -> (Option<Square>, u64, u32) {
        let prev_ep = self.en_passant_target; let prev_hash = self.hash; let prev_halfmove = self.halfmove_clock;
        if let Some(ep) = self.en_passant_target { self.hash ^= ZOBRIST.en_passant_keys[ep.1]; }
        self.white_to_move = !self.white_to_move; self.hash ^= ZOBRIST.black_to_move_key; self.en_passant_target = None; self.halfmove_clock = self.halfmove_clock.saturating_add(1); self.position_history.push(self.hash);
        (prev_ep, prev_hash, prev_halfmove)
    }

    pub(crate) fn unmake_null_move(&mut self, prev_ep: Option<Square>, prev_hash: u64, prev_halfmove: u32) {
        let _ = self.position_history.pop(); self.hash = prev_hash; self.white_to_move = !self.white_to_move; self.en_passant_target = prev_ep; self.halfmove_clock = prev_halfmove;
    }

    fn generate_pseudo_moves(&self) -> Vec<Move> {
        self.generate_pseudo_moves_bb()
    }

    fn generate_pseudo_moves_bb(&self) -> Vec<Move> {
        use crate::bitboards as bb;
        let mut moves = Vec::new();
        let color = self.current_color();
        let us = color;
        let _them = self.opponent_color(color);

        // Use existing bitboards
        let occ = self.all_pieces();
        let occ_us = self.pieces_of_color(us);
        let occ_them = self.pieces_of_color(self.opponent_color(us));
        let (pawns, knights, bishops, rooks, queens, king) = match us {
            Color::White => (self.white_pawns, self.white_knights, self.white_bishops, self.white_rooks, self.white_queens, self.white_king),
            Color::Black => (self.black_pawns, self.black_knights, self.black_bishops, self.black_rooks, self.black_queens, self.black_king),
        };

        // Helper to add a quiet or capture move from bit indices
        let mut push_move = |from_idx: usize, to_idx: usize, promo: Option<Piece>, is_ep: bool| {
            let from = Square(from_idx/8, from_idx%8);
            let to = Square(to_idx/8, to_idx%8);
            let captured_piece = if is_ep { Some(Piece::Pawn) } else { self.piece_at(to).map(|(_,p)| p) };
            moves.push(Move{ from, to, is_castling:false, is_en_passant:is_ep, promotion:promo, captured_piece });
        };

        // Knights first - prioritize piece development over pawn pushes
        let mut n = knights; while n!=0 { let from = n.trailing_zeros() as usize; n &= n-1; let from_sq = Square(from/8, from%8); let mut targets = bb::knight_attacks_from(from_sq) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); } }

        // Bishops second  
        let mut b = bishops; while b!=0 { let from = b.trailing_zeros() as usize; b &= b-1; let from_sq = Square(from/8, from%8); let mut targets = bb::bishop_attacks_from(from_sq, occ) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); } }

        // Pawn captures first (higher priority than quiet pawn moves)
        if pawns != 0 {
            if us == Color::White {
                // Captures first
                let left_capt = (pawns << 7) & !bb::FILE_H & occ_them;
                let right_capt = (pawns << 9) & !bb::FILE_A & occ_them;
                let mut lc = left_capt; while lc!=0 { let to = lc.trailing_zeros() as usize; lc &= lc-1; let from = to-7; if to>=56 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight]{ push_move(from,to,Some(promo),false);} } else { push_move(from,to,None,false);} }
                let mut rc = right_capt; while rc!=0 { let to = rc.trailing_zeros() as usize; rc &= rc-1; let from = to-9; if to>=56 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight]{ push_move(from,to,Some(promo),false);} } else { push_move(from,to,None,false);} }
                // En passant
                if let Some(ep) = self.en_passant_target { let ep_bb = bb::sq_to_bb(ep); let left_ep = (pawns << 7) & !bb::FILE_H & ep_bb; if left_ep!=0 { let to = ep.0*8+ep.1; let from = to-7; push_move(from,to,None,true); } let right_ep = (pawns << 9) & !bb::FILE_A & ep_bb; if right_ep!=0 { let to = ep.0*8+ep.1; let from = to-9; push_move(from,to,None,true); } }
                // Pushes second
                let empty = !occ;
                let one = (pawns << 8) & empty;
                // Promotions on rank 8
                let promos = one & bb::RANK8;
                let quiets = one & !bb::RANK8;
                let mut q = quiets; while q!=0 { let to = q.trailing_zeros() as usize; q &= q-1; let from = to-8; push_move(from, to, None, false); }
                let mut pr = promos; while pr!=0 { let to = pr.trailing_zeros() as usize; pr &= pr-1; let from = to-8; for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight] { push_move(from,to,Some(promo),false); } }
                // Double pushes from rank2
                let two = ((one & bb::rank3_mask()) << 8) & empty; // rank3_mask computed below via helper
                let mut t = two; while t!=0 { let to = t.trailing_zeros() as usize; t &= t-1; let from = to-16; push_move(from, to, None, false); }
            } else {
                // Black - captures first
                let left_capt = (pawns >> 9) & !bb::FILE_H & occ_them;
                let right_capt = (pawns >> 7) & !bb::FILE_A & occ_them;
                let mut lc = left_capt; while lc!=0 { let to = lc.trailing_zeros() as usize; lc &= lc-1; let from = to+9; if to<8 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight]{ push_move(from,to,Some(promo),false);} } else { push_move(from,to,None,false);} }
                let mut rc = right_capt; while rc!=0 { let to = rc.trailing_zeros() as usize; rc &= rc-1; let from = to+7; if to<8 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight]{ push_move(from,to,Some(promo),false);} } else { push_move(from,to,None,false);} }
                // EP
                if let Some(ep) = self.en_passant_target { let ep_bb = bb::sq_to_bb(ep); let left_ep = (pawns >> 9) & !bb::FILE_H & ep_bb; if left_ep!=0 { let to = ep.0*8+ep.1; let from = to+9; push_move(from,to,None,true); } let right_ep = (pawns >> 7) & !bb::FILE_A & ep_bb; if right_ep!=0 { let to = ep.0*8+ep.1; let from = to+7; push_move(from,to,None,true); } }
                // Pushes second
                let empty = !occ;
                let one = (pawns >> 8) & empty;
                let promos = one & bb::RANK1;
                let quiets = one & !bb::RANK1;
                let mut q = quiets; while q!=0 { let to = q.trailing_zeros() as usize; q &= q-1; let from = to+8; push_move(from, to, None, false); }
                let mut pr = promos; while pr!=0 { let to = pr.trailing_zeros() as usize; pr &= pr-1; let from = to+8; for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight] { push_move(from,to,Some(promo),false); } }
                let two = ((one & bb::rank6_mask()) >> 8) & empty;
                let mut t = two; while t!=0 { let to = t.trailing_zeros() as usize; t &= t-1; let from = to+16; push_move(from, to, None, false); }
            }
        }

        // Rooks
        let mut r = rooks; while r!=0 { let from = r.trailing_zeros() as usize; r &= r-1; let from_sq = Square(from/8, from%8); let mut targets = bb::rook_attacks_from(from_sq, occ) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); } }

        // Queens
        let mut qn = queens; while qn!=0 { let from = qn.trailing_zeros() as usize; qn &= qn-1; let from_sq = Square(from/8, from%8); let mut targets = (bb::rook_attacks_from(from_sq, occ) | bb::bishop_attacks_from(from_sq, occ)) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); } }

        // King (including castling squares without safety check here; legality filter later will exclude illegal castling through check)
        if king != 0 { let from = king.trailing_zeros() as usize; let from_sq = Square(from/8, from%8); let mut targets = bb::king_attacks_from(from_sq) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); }
            // Castling pseudo-moves
            let back_rank = if us==Color::White {0} else {7};
            if from_sq == Square(back_rank,4) {
                // King side
                if self.castling_rights.contains(&(us,'K')) && self.piece_at(Square(back_rank,5)).is_none() && self.piece_at(Square(back_rank,6)).is_none() && self.piece_at(Square(back_rank,7))==Some((us,Piece::Rook)) {
                    moves.push(Move{ from:from_sq, to:Square(back_rank,6), is_castling:true, is_en_passant:false, promotion:None, captured_piece:None});
                }
                // Queen side
                if self.castling_rights.contains(&(us,'Q')) && self.piece_at(Square(back_rank,1)).is_none() && self.piece_at(Square(back_rank,2)).is_none() && self.piece_at(Square(back_rank,3)).is_none() && self.piece_at(Square(back_rank,0))==Some((us,Piece::Rook)) {
                    moves.push(Move{ from:from_sq, to:Square(back_rank,2), is_castling:true, is_en_passant:false, promotion:None, captured_piece:None});
                }
            }
        }

        moves
    }

    pub(crate) fn generate_piece_moves(&self, from: Square, piece: Piece) -> Vec<Move> {
        use crate::bitboards as bb;
        // Build occupancy masks
        let mut occ: u64 = 0; let mut occ_us: u64 = 0;
        let us = self.current_color();
        let occ = self.all_pieces();
        let occ_us = self.pieces_of_color(us);

        match piece {
            Piece::Pawn => self.generate_pawn_moves(from),
            Piece::Knight => {
                let mut moves = Vec::new();
                let mut targets = bb::knight_attacks_from(from) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                moves
            }
            Piece::Bishop => {
                let mut moves = Vec::new();
                let mut targets = bb::bishop_attacks_from(from, occ) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                moves
            }
            Piece::Rook => {
                let mut moves = Vec::new();
                let mut targets = bb::rook_attacks_from(from, occ) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                moves
            }
            Piece::Queen => {
                let mut moves = Vec::new();
                let mut targets = (bb::rook_attacks_from(from, occ) | bb::bishop_attacks_from(from, occ)) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                moves
            }
            Piece::King => {
                // Single king moves via bitboards + castling pseudo as before
                let mut moves = Vec::new();
                let mut targets = bb::king_attacks_from(from) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                // Castling pseudo-moves
                let color = self.current_color();
                let back_rank = if color == Color::White { 0 } else { 7 };
                if from == Square(back_rank, 4) {
                    if self.castling_rights.contains(&(color, 'K')) && self.piece_at(Square(back_rank,5)).is_none() && self.piece_at(Square(back_rank,6)).is_none() && self.piece_at(Square(back_rank,7)) == Some((color, Piece::Rook)) {
                        moves.push(Move{ from, to:Square(back_rank,6), is_castling:true, is_en_passant:false, promotion:None, captured_piece:None});
                    }
                    if self.castling_rights.contains(&(color, 'Q')) && self.piece_at(Square(back_rank,1)).is_none() && self.piece_at(Square(back_rank,2)).is_none() && self.piece_at(Square(back_rank,3)).is_none() && self.piece_at(Square(back_rank,0)) == Some((color, Piece::Rook)) {
                        moves.push(Move{ from, to:Square(back_rank,2), is_castling:true, is_en_passant:false, promotion:None, captured_piece:None});
                    }
                }
                moves
            }
        }
    }

    fn create_move(&self, from: Square, to: Square, promotion: Option<Piece>, is_castling: bool, is_en_passant: bool) -> Move {
        let captured_piece = if is_en_passant { Some(Piece::Pawn) } else if !is_castling { self.piece_at(to).map(|(_, p)| p) } else { None };
        Move { from, to, promotion, is_castling, is_en_passant, captured_piece }
    }

    fn generate_pawn_moves(&self, from: Square) -> Vec<Move> {
        let color = if self.white_to_move { Color::White } else { Color::Black };
        let mut moves = Vec::new();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let start_rank = if color == Color::White { 1 } else { 6 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };
        let r = from.0 as isize; let f = from.1 as isize;
        let forward_r = r + dir;
        if forward_r >= 0 && forward_r < 8 { let forward_sq = Square(forward_r as usize, f as usize); if self.piece_at(forward_sq).is_none() { if forward_sq.0 == promotion_rank { for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] { moves.push(self.create_move(from, forward_sq, Some(promo), false, false)); } } else { moves.push(self.create_move(from, forward_sq, None, false, false)); if r == start_rank as isize { let double_forward_r = r + 2 * dir; let double_forward_sq = Square(double_forward_r as usize, f as usize); if self.piece_at(double_forward_sq).is_none() { moves.push(self.create_move(from, double_forward_sq, None, false, false)); } } } } }
        if forward_r >= 0 && forward_r < 8 { for df in [-1, 1] { let capture_f = f + df; if capture_f >= 0 && capture_f < 8 { let target_sq = Square(forward_r as usize, capture_f as usize); if let Some((target_color, _)) = self.piece_at(target_sq) { if target_color != color { if target_sq.0 == promotion_rank { for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] { moves.push(self.create_move(from, target_sq, Some(promo), false, false)); } } else { moves.push(self.create_move(from, target_sq, None, false, false)); } } } else if Some(target_sq) == self.en_passant_target { moves.push(self.create_move(from, target_sq, None, false, true)); } } } }
        moves
    }

    pub(crate) fn generate_pawn_tactical_moves(&self, from: Square, out: &mut Vec<Move>) {
        use crate::bitboards as bb;
        let color = if self.white_to_move { Color::White } else { Color::Black };
        // Build occupancy of opponents to detect captures
        let occ_them = self.pieces_of_color(self.opponent_color(color));

        let from_idx = from.0*8 + from.1;
        if color == Color::White {
            // White captures: left (<<7, not on H-file), right (<<9, not on A-file)
            let left = ((1u64 << from_idx) << 7) & !bb::FILE_H & occ_them;
            let right = ((1u64 << from_idx) << 9) & !bb::FILE_A & occ_them;
            let mut caps = left | right;
            while caps != 0 { let to = caps.trailing_zeros() as usize; caps &= caps-1; let to_sq = Square(to/8,to%8); if to_sq.0 == 7 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight] { out.push(self.create_move(from, to_sq, Some(promo), false, false)); } } else { out.push(self.create_move(from, to_sq, None, false, false)); } }
            // En passant
            if let Some(ep) = self.en_passant_target { let ep_bb = bb::sq_to_bb(ep); let left_ep = ((1u64<<from_idx) << 7) & !bb::FILE_H & ep_bb; if left_ep!=0 { out.push(self.create_move(from, ep, None, false, true)); } let right_ep = ((1u64<<from_idx) << 9) & !bb::FILE_A & ep_bb; if right_ep!=0 { out.push(self.create_move(from, ep, None, false, true)); } }
        } else {
            // Black captures
            let left = ((1u64 << from_idx) >> 9) & !bb::FILE_H & occ_them;
            let right = ((1u64 << from_idx) >> 7) & !bb::FILE_A & occ_them;
            let mut caps = left | right;
            while caps != 0 { let to = caps.trailing_zeros() as usize; caps &= caps-1; let to_sq = Square(to/8,to%8); if to_sq.0 == 0 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight] { out.push(self.create_move(from, to_sq, Some(promo), false, false)); } } else { out.push(self.create_move(from, to_sq, None, false, false)); } }
            if let Some(ep) = self.en_passant_target { let ep_bb = bb::sq_to_bb(ep); let left_ep = ((1u64<<from_idx) >> 9) & !bb::FILE_H & ep_bb; if left_ep!=0 { out.push(self.create_move(from, ep, None, false, true)); } let right_ep = ((1u64<<from_idx) >> 7) & !bb::FILE_A & ep_bb; if right_ep!=0 { out.push(self.create_move(from, ep, None, false, true)); } }
        }
    }

    fn generate_knight_moves(&self, from: Square) -> Vec<Move> {
        let mut moves = Vec::new();
        let deltas = [(2, 1),(1, 2),(-1, 2),(-2, 1),(-2, -1),(-1, -2),(1, -2),(2, -1)];
        let (rank, file) = (from.0 as isize, from.1 as isize);
        let color = self.current_color();
        for (dr, df) in deltas { let (nr, nf) = (rank + dr, file + df); if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 { let to_sq = Square(nr as usize, nf as usize); if let Some((c, _)) = self.piece_at(to_sq) { if c != color { moves.push(self.create_move(from, to_sq, None, false, false)); } } else { moves.push(self.create_move(from, to_sq, None, false, false)); } } }
        moves
    }

    fn generate_sliding_moves(&self, from: Square, directions: &[(isize, isize)]) -> Vec<Move> {
        let mut moves = Vec::new();
        let (rank, file) = (from.0 as isize, from.1 as isize);
        let color = self.current_color();
        for &(dr, df) in directions { let mut r = rank + dr; let mut f = file + df; while r >= 0 && r < 8 && f >= 0 && f < 8 { let to_sq = Square(r as usize, f as usize); if let Some((c, _)) = self.piece_at(to_sq) { if c != color { moves.push(self.create_move(from, to_sq, None, false, false)); } break; } else { moves.push(self.create_move(from, to_sq, None, false, false)); } r += dr; f += df; } }
        moves
    }

    fn generate_king_moves(&self, from: Square) -> Vec<Move> {
        let mut moves = Vec::new();
        let deltas = [(1, 0),(-1, 0),(0, 1),(0, -1),(1, 1),(1, -1),(-1, 1),(-1, -1)];
        let (rank, file) = (from.0, from.1);
        let color = self.current_color();
        let back_rank = if color == Color::White { 0 } else { 7 };
        for (dr, df) in deltas { let (nr, nf) = (rank as isize + dr, file as isize + df); if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 { let to_sq = Square(nr as usize, nf as usize); if let Some((c, _)) = self.piece_at(to_sq) { if c != color { moves.push(self.create_move(from, to_sq, None, false, false)); } } else { moves.push(self.create_move(from, to_sq, None, false, false)); } } }
        if from == Square(back_rank, 4) {
            if self.castling_rights.contains(&(color, 'K')) && self.piece_at(Square(back_rank, 5)).is_none() && self.piece_at(Square(back_rank, 6)).is_none() && self.piece_at(Square(back_rank, 7)) == Some((color, Piece::Rook)) {
                let to_sq = Square(back_rank, 6); moves.push(self.create_move(from, to_sq, None, true, false));
            }
            if self.castling_rights.contains(&(color, 'Q')) && self.piece_at(Square(back_rank, 1)).is_none() && self.piece_at(Square(back_rank, 2)).is_none() && self.piece_at(Square(back_rank, 3)).is_none() && self.piece_at(Square(back_rank, 0)) == Some((color, Piece::Rook)) {
                let to_sq = Square(back_rank, 2); moves.push(self.create_move(from, to_sq, None, true, false));
            }
        }
        moves
    }

    fn find_king(&self, color: Color) -> Option<Square> {
        let king_bb = if color == Color::White { self.white_king } else { self.black_king };
        if king_bb != 0 {
            let sq_idx = king_bb.trailing_zeros() as usize;
            Some(Square(sq_idx / 8, sq_idx % 8))
        } else {
            None
        }
    }

    pub(crate) fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        crate::bitboards::is_square_attacked_bb(self, square, attacker_color)
    }

    pub(crate) fn is_in_check(&self, color: Color) -> bool { if let Some(king_sq) = self.find_king(color) { self.is_square_attacked(king_sq, self.opponent_color(color)) } else { false } }
    pub(crate) fn is_fifty_move_draw(&self) -> bool { self.halfmove_clock >= 100 }
    pub(crate) fn is_threefold_repetition(&self) -> bool { let current = self.hash; let mut count = 0; for &h in &self.position_history { if h == current { count += 1; } } count >= 3 }

    pub(crate) fn generate_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color(); let opponent_color = self.opponent_color(current_color); let pseudo_moves = self.generate_pseudo_moves(); let mut legal_moves = Vec::new();
        for m in pseudo_moves { if m.is_castling { let king_start_sq = m.from; let king_mid_sq = Square(m.from.0, (m.from.1 + m.to.1) / 2); let king_end_sq = m.to; if self.is_square_attacked(king_start_sq, opponent_color) || self.is_square_attacked(king_mid_sq, opponent_color) || self.is_square_attacked(king_end_sq, opponent_color) { continue; } }
            let info = self.make_move(&m); if !self.is_in_check(current_color) { legal_moves.push(m.clone()); } self.unmake_move(&m, info);
        }
        legal_moves
    }

    pub(crate) fn generate_tactical_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color(); let opponent_color = self.opponent_color(current_color); let pseudo_moves = self.generate_pseudo_moves(); let mut tactical_moves = Vec::new();
        for m in pseudo_moves {
            // Include captures, promotions, and checking moves
            let is_capture = m.captured_piece.is_some();
            let is_promotion = m.promotion.is_some();
            let is_check = {
                let info = self.make_move(&m);
                let gives_check = self.is_in_check(opponent_color);
                self.unmake_move(&m, info);
                gives_check
            };
            
            if is_capture || is_promotion || is_check {
                let info = self.make_move(&m);
                if !self.is_in_check(current_color) { tactical_moves.push(m.clone()); }
                self.unmake_move(&m, info);
            }
        }
        tactical_moves
    }

    pub(crate) fn is_checkmate(&mut self) -> bool { let color = self.current_color(); self.is_in_check(color) && self.generate_moves().is_empty() }
    pub(crate) fn is_stalemate(&mut self) -> bool { let color = self.current_color(); !self.is_in_check(color) && self.generate_moves().is_empty() }

    pub(crate) fn extract_pv(&self, tt: &TranspositionTable, max_depth: usize) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut current_hash = self.hash;
        let mut visited_positions = std::collections::HashSet::new();
        
        for _depth in 0..max_depth {
            if visited_positions.contains(&current_hash) {
                break; // Avoid cycles
            }
            visited_positions.insert(current_hash);
            
            if let Some(entry) = tt.probe(current_hash) {
                if let Some(mv) = &entry.best_move {
                    pv.push(*mv);
                    // Update hash for next iteration (simplified)
                    current_hash = current_hash.wrapping_add(mv.from.0 as u64 + mv.to.0 as u64);
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        
        pv
    }

    pub(crate) fn evaluate(&self) -> i32 {
        let mut score = 0;
        const MATERIAL_MG: [i32; 6] = [82, 337, 365, 477, 1025, 20000];
        const MATERIAL_EG: [i32; 6] = [94, 281, 297, 512, 936, 20000];
        const PST_MG: [[i32; 64]; 6] = [
            [0,0,0,0,0,0,0,0,-35,-1,-20,-23,-15,24,38,-22,-26,-4,-4,-10,3,3,33,-12,-27,-2,-5,12,17,6,10,-25,-14,13,6,21,23,12,17,-23,-6,7,26,31,65,56,25,-20,98,134,61,95,68,126,34,-11,0,0,0,0,0,0,0,0],
            [-105,-21,-58,-33,-17,-28,-19,-23,-29,-53,-12,-3,-1,18,-14,-19,-23,-9,12,10,19,17,25,-16,-13,4,16,13,28,19,21,-8,-9,17,19,53,37,69,18,22,-47,60,37,65,84,129,73,44,-73,-41,72,36,23,62,7,-17,-167,-89,-34,-49,61,-97,-15,-107],
            [-33,-3,-14,-21,-13,-12,-39,-21,4,15,16,0,7,21,33,1,0,15,15,15,14,27,18,10,-6,13,13,26,34,12,10,4,-4,5,19,50,37,37,7,-2,-16,37,43,40,35,50,37,-2,-26,16,-18,-13,30,59,18,-47,-29,4,-82,-37,-25,-42,7,-8],
            [-19,-13,1,17,16,7,-37,-26,-44,-16,-20,-9,-1,11,-6,-71,-45,-25,-16,-17,3,0,-5,-33,-36,-26,-12,-1,9,-7,6,-23,-24,-11,7,26,24,35,-8,-20,-5,19,26,36,17,45,61,16,27,32,58,62,80,67,26,44,32,42,32,51,63,9,31,43],
            [-1,-18,-9,10,-15,-25,-31,-50,-35,-8,11,2,8,15,-3,1,-14,2,-11,-2,-5,2,14,5,-9,-26,-9,-10,-2,-4,3,-3,-27,-27,-16,-16,-1,17,-2,1,-13,-17,7,8,29,56,47,57,-24,-39,-5,1,-16,57,28,54,-28,0,29,12,59,44,43,45],
            [-15,36,12,-54,8,-28,34,14,1,7,-8,-64,-43,-16,9,8,-14,-14,-22,-46,-44,-30,-15,-27,-49,-1,-27,-39,-46,-44,-33,-51,-17,-20,-12,-27,-30,-25,-14,-36,-9,24,2,-16,-20,6,22,-22,29,-1,-20,-7,-8,-4,-38,-29,-65,23,16,-15,-56,-34,2,13]
        ];
        const PST_EG: [[i32; 64]; 6] = [
            [0,0,0,0,0,0,0,0,13,8,8,10,13,0,2,-7,4,7,-6,1,0,-5,-1,-8,13,9,-3,-7,-7,-8,3,-1,32,24,13,5,-2,4,17,17,94,100,85,67,56,53,82,84,178,173,158,134,147,132,165,187,0,0,0,0,0,0,0,0],
            [-29,-51,-23,-15,-22,-18,-50,-64,-42,-20,-10,-5,-2,-20,-23,-44,-23,-3,-1,15,10,-3,-20,-22,-18,-6,16,25,16,17,4,-18,-17,3,22,22,22,11,8,-18,-24,-20,10,9,-1,-9,-19,-41,-25,-8,-25,-2,-9,-25,-24,-52,-58,-38,-13,-28,-31,-27,-63,-99],
            [-23,-9,-23,-5,-9,-16,-5,-17,-14,-18,-7,-1,4,-9,-15,-27,-12,-3,8,10,13,3,-7,-15,-6,3,13,19,7,10,-3,-9,-3,9,12,9,14,10,3,2,2,-8,0,-1,-2,6,0,4,-8,-4,7,-12,-3,-13,-4,-14,-14,-21,-11,-8,-7,-9,-17,-24],
            [-9,2,3,-1,-5,-13,4,-20,-6,-6,0,2,-9,-9,-11,-3,-4,0,-5,-1,-7,-12,-8,-16,3,5,8,4,-5,-6,-8,-11,4,3,13,1,2,1,-1,2,7,7,7,5,4,-3,-5,-3,11,13,13,11,-3,3,8,3,13,10,18,15,12,12,8,5],
            [-33,-28,-22,-43,-5,-32,-20,-41,-22,-23,-30,-16,-16,-23,-36,-32,-16,-27,15,6,9,17,10,5,-18,28,19,47,31,34,39,23,3,22,24,45,57,40,57,36,-20,6,9,49,47,35,19,9,-17,20,32,41,58,25,30,0,-9,22,22,27,27,19,10,20],
            [-53,-34,-21,-11,-28,-14,-24,-43,-27,-11,4,13,14,4,-5,-17,-19,-3,11,21,23,16,7,-9,-18,-4,21,24,27,23,9,-11,-8,22,24,27,26,33,26,3,10,17,23,15,20,45,44,13,-12,17,14,17,17,38,23,11,-74,-35,-18,-18,-11,15,4,-17]
        ];
        fn square_to_index(rank: usize, file: usize) -> usize { rank * 8 + file }
        fn piece_to_index(piece: Piece) -> usize { match piece { Piece::Pawn => 0, Piece::Knight => 1, Piece::Bishop => 2, Piece::Rook => 3, Piece::Queen => 4, Piece::King => 5 } }
        let mut white_material_mg = 0; let mut black_material_mg = 0; let mut _white_material_eg = 0; let mut _black_material_eg = 0; let mut white_bishop_count = 0; let mut black_bishop_count = 0; let mut white_pawns_by_file = [0; 8]; let mut black_pawns_by_file = [0; 8];
        // Count pieces and material using bitboards
        white_bishop_count = self.white_bishops.count_ones() as usize;
        black_bishop_count = self.black_bishops.count_ones() as usize;
        
        // Count material
        white_material_mg += self.white_pawns.count_ones() as i32 * MATERIAL_MG[0];
        white_material_mg += self.white_knights.count_ones() as i32 * MATERIAL_MG[1];
        white_material_mg += self.white_bishops.count_ones() as i32 * MATERIAL_MG[2];
        white_material_mg += self.white_rooks.count_ones() as i32 * MATERIAL_MG[3];
        white_material_mg += self.white_queens.count_ones() as i32 * MATERIAL_MG[4];
        
        black_material_mg += self.black_pawns.count_ones() as i32 * MATERIAL_MG[0];
        black_material_mg += self.black_knights.count_ones() as i32 * MATERIAL_MG[1];
        black_material_mg += self.black_bishops.count_ones() as i32 * MATERIAL_MG[2];
        black_material_mg += self.black_rooks.count_ones() as i32 * MATERIAL_MG[3];
        black_material_mg += self.black_queens.count_ones() as i32 * MATERIAL_MG[4];
        
        // Count pawns by file
        let mut wp = self.white_pawns;
        while wp != 0 {
            let sq_idx = wp.trailing_zeros() as usize;
            wp &= wp - 1;
            let file = sq_idx % 8;
            white_pawns_by_file[file] += 1;
        }
        let mut bp = self.black_pawns;
        while bp != 0 {
            let sq_idx = bp.trailing_zeros() as usize;
            bp &= bp - 1;
            let file = sq_idx % 8;
            black_pawns_by_file[file] += 1;
        }
        let total_material_mg = white_material_mg + black_material_mg;
        let max_material = 2 * (MATERIAL_MG[1]*2 + MATERIAL_MG[2]*2 + MATERIAL_MG[3]*2 + MATERIAL_MG[4] + MATERIAL_MG[0]*8);
        let phase = (total_material_mg as f32) / (max_material as f32);
        let phase = phase.min(1.0).max(0.0);
        let mut mg_score = 0; let mut eg_score = 0;
        // PST evaluation using bitboards
        let pieces = [
            (self.white_pawns, Color::White, 0),
            (self.white_knights, Color::White, 1),
            (self.white_bishops, Color::White, 2),
            (self.white_rooks, Color::White, 3),
            (self.white_queens, Color::White, 4),
            (self.white_king, Color::White, 5),
            (self.black_pawns, Color::Black, 0),
            (self.black_knights, Color::Black, 1),
            (self.black_bishops, Color::Black, 2),
            (self.black_rooks, Color::Black, 3),
            (self.black_queens, Color::Black, 4),
            (self.black_king, Color::Black, 5),
        ];
        
        for (mut bb, color, piece_idx) in pieces {
            while bb != 0 {
                let sq_idx_raw = bb.trailing_zeros() as usize;
                bb &= bb - 1;
                let rank = sq_idx_raw / 8;
                let file = sq_idx_raw % 8;
                // PST tables are defined from White's perspective (rank 0 = White back rank).
                // Therefore: use (rank,file) directly for White, and mirror vertically for Black.
                let sq_idx = if color == Color::White {
                    square_to_index(rank, file)
                } else {
                    square_to_index(7 - rank, file)
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
        let position_score = (phase * mg_score as f32 + (1.0 - phase) * eg_score as f32) as i32; score += position_score;
        if white_bishop_count >= 2 { score += 30; } if black_bishop_count >= 2 { score -= 30; }
        // Rook evaluation using bitboards
        let mut wr = self.white_rooks;
        while wr != 0 {
            let sq_idx = wr.trailing_zeros() as usize;
            wr &= wr - 1;
            let file = sq_idx % 8;
            let file_pawns = white_pawns_by_file[file] + black_pawns_by_file[file];
            if file_pawns == 0 {
                score += 15; // Open file bonus
            } else if black_pawns_by_file[file] == 0 {
                score += 7; // Semi-open file bonus
            }
        }
        let mut br = self.black_rooks;
        while br != 0 {
            let sq_idx = br.trailing_zeros() as usize;
            br &= br - 1;
            let file = sq_idx % 8;
            let file_pawns = white_pawns_by_file[file] + black_pawns_by_file[file];
            if file_pawns == 0 {
                score -= 15; // Open file bonus
            } else if white_pawns_by_file[file] == 0 {
                score -= 7; // Semi-open file bonus
            }
        }
        for file in 0..8 { if white_pawns_by_file[file] > 0 { let left_file = if file>0 { white_pawns_by_file[file-1] } else { 0 }; let right_file = if file<7 { white_pawns_by_file[file+1] } else { 0 }; if left_file==0 && right_file==0 { score -= 12; } }
            if black_pawns_by_file[file] > 0 { let left_file = if file>0 { black_pawns_by_file[file-1] } else { 0 }; let right_file = if file<7 { black_pawns_by_file[file+1] } else { 0 }; if left_file==0 && right_file==0 { score += 12; } }
            // Passed pawn evaluation using bitboards
            let file_mask = 1u64 << file | if file > 0 { 1u64 << (file - 1) } else { 0 } | if file < 7 { 1u64 << (file + 1) } else { 0 };
            
            // Check white pawns on this file
            let white_pawns_file = self.white_pawns & (0x0101010101010101u64 << file);
            let mut wp_file = white_pawns_file;
            while wp_file != 0 {
                let sq_idx = wp_file.trailing_zeros() as usize;
                wp_file &= wp_file - 1;
                let rank = sq_idx / 8;
                
                // Check if this pawn is passed (no enemy pawns ahead in adjacent files)
                let ahead_mask = if rank == 0 { 0 } else { 
                    let mut mask = 0u64;
                    for check_file in file.saturating_sub(1)..=(file+1).min(7) {
                        for check_rank in 0..rank {
                            mask |= 1u64 << (check_rank * 8 + check_file);
                        }
                    }
                    mask
                };
                if (self.black_pawns & ahead_mask) == 0 {
                    let bonus = 10 + (7 - rank as i32) * 7;
                    score += bonus;
                }
            }
            
            // Check black pawns on this file
            let black_pawns_file = self.black_pawns & (0x0101010101010101u64 << file);
            let mut bp_file = black_pawns_file;
            while bp_file != 0 {
                let sq_idx = bp_file.trailing_zeros() as usize;
                bp_file &= bp_file - 1;
                let rank = sq_idx / 8;
                
                // Check if this pawn is passed (no enemy pawns ahead in adjacent files)
                let ahead_mask = if rank == 7 { 0 } else { 
                    let mut mask = 0u64;
                    for check_file in file.saturating_sub(1)..=(file+1).min(7) {
                        for check_rank in (rank+1)..8 {
                            mask |= 1u64 << (check_rank * 8 + check_file);
                        }
                    }
                    mask
                };
                if (self.white_pawns & ahead_mask) == 0 {
                    let bonus = 10 + rank as i32 * 7;
                    score -= bonus;
                }
            }
        }
        if self.white_to_move { score } else { -score }
    }

    // --- SEE helpers and SEE ---
    #[allow(dead_code)]
    fn attackers_to_square(&self, target: Square, color: Color, occ: &[[Option<(Color, Piece)>; 8]; 8]) -> Vec<(Square, Piece)> {
        let mut attackers = Vec::new(); let (tr, tf) = (target.0 as isize, target.1 as isize);
        let pawn_dir: isize = if color == Color::White { -1 } else { 1 };
        for df in [-1, 1] { let r = tr + pawn_dir; let f = tf + df; if r>=0 && r<8 && f>=0 && f<8 { if occ[r as usize][f as usize] == Some((color, Piece::Pawn)) { attackers.push((Square(r as usize, f as usize), Piece::Pawn)); } } }
        let knight_deltas = [(2,1),(1,2),(-1,2),(-2,1),(-2,-1),(-1,-2),(1,-2),(2,-1)];
        for (dr, df) in knight_deltas { let r = tr+dr; let f = tf+df; if r>=0 && r<8 && f>=0 && f<8 { if occ[r as usize][f as usize] == Some((color, Piece::Knight)) { attackers.push((Square(r as usize, f as usize), Piece::Knight)); } } }
        let king_deltas = [(1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)];
        for (dr, df) in king_deltas { let r = tr+dr; let f = tf+df; if r>=0 && r<8 && f>=0 && f<8 { if occ[r as usize][f as usize] == Some((color, Piece::King)) { attackers.push((Square(r as usize, f as usize), Piece::King)); } } }
        let rook_dirs = [(1,0),(-1,0),(0,1),(0,-1)];
        for (dr, df) in rook_dirs { let mut r = tr+dr; let mut f = tf+df; while r>=0 && r<8 && f>=0 && f<8 { if let Some((c,p)) = occ[r as usize][f as usize] { if c==color { if p==Piece::Rook || p==Piece::Queen { attackers.push((Square(r as usize, f as usize), p)); } } break; } r+=dr; f+=df; } }
        let bishop_dirs = [(1,1),(1,-1),(-1,1),(-1,-1)];
        for (dr, df) in bishop_dirs { let mut r = tr+dr; let mut f = tf+df; while r>=0 && r<8 && f>=0 && f<8 { if let Some((c,p)) = occ[r as usize][f as usize] { if c==color { if p==Piece::Bishop || p==Piece::Queen { attackers.push((Square(r as usize, f as usize), p)); } } break; } r+=dr; f+=df; } }
        attackers
    }

    pub(crate) fn see(&self, m: &Move) -> i32 {
        if !(m.captured_piece.is_some() || m.is_en_passant) { return 0; }
        
        // Create a temporary board copy for SEE calculation
        let mut temp_board = Board {
            white_pawns: self.white_pawns,
            white_knights: self.white_knights,
            white_bishops: self.white_bishops,
            white_rooks: self.white_rooks,
            white_queens: self.white_queens,
            white_king: self.white_king,
            black_pawns: self.black_pawns,
            black_knights: self.black_knights,
            black_bishops: self.black_bishops,
            black_rooks: self.black_rooks,
            black_queens: self.black_queens,
            black_king: self.black_king,
            white_to_move: self.white_to_move,
            en_passant_target: self.en_passant_target,
            castling_rights: self.castling_rights.clone(),
            hash: self.hash,
            halfmove_clock: self.halfmove_clock,
            position_history: self.position_history.clone(),
        };
        
        let (attacker_color, mut moving_piece) = self.piece_at(m.from).unwrap();
        if let Some(promo) = m.promotion { moving_piece = promo; }
        let target = m.to;
        let captured_value: i32 = if m.is_en_passant { 
            let cap_row = if attacker_color == Color::White { m.to.0 - 1 } else { m.to.0 + 1 };
            temp_board.clear_square(Square(cap_row, m.to.1));
            piece_value(Piece::Pawn) 
        } else { 
            m.captured_piece.map(piece_value).unwrap_or(0) 
        };
        temp_board.clear_square(m.from);
        temp_board.set_piece_at(target, attacker_color, moving_piece);
        let mut gains: [i32; 32] = [0; 32]; let mut d = 0usize; gains[d] = captured_value; let mut stm = self.opponent_color(attacker_color);
        let select_lva = |list: &[(Square, Piece)]| -> Option<(Square, Piece)> {
            list
                .iter()
                .min_by_key(|(_, p)| piece_value(*p))
                .map(|(sq, p)| (*sq, *p))
        };
        // Simplified SEE - just return the captured piece value for now
        // TODO: Implement full SEE with bitboard-based attacker detection
        captured_value
    }

    pub(crate) fn perft(&mut self, depth: usize) -> u64 {
        if depth == 0 { return 1; }
        let moves = self.generate_moves(); if depth == 1 { return moves.len() as u64; }
        let mut nodes = 0; for m in moves { let info = self.make_move(&m); nodes += self.perft(depth - 1); self.unmake_move(&m, info); }
        nodes
    }

    pub(crate) fn current_color(&self) -> Color { if self.white_to_move { Color::White } else { Color::Black } }
    pub(crate) fn opponent_color(&self, color: Color) -> Color { match color { Color::White => Color::Black, Color::Black => Color::White } }
    
    // Methods needed by publius search
    pub(crate) fn get_opposite_color(&self, color: Color) -> Color {
        self.opponent_color(color)
    }
    
    pub(crate) fn is_draw(&self) -> bool {
        self.is_fifty_move_draw() || self.is_threefold_repetition()
    }
    


    #[allow(dead_code)]
    pub(crate) fn print(&self) {
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() { print!("{} |", rank + 1); for file in 0..8 { let piece_char = match self.piece_at(Square(rank, file)) { Some((Color::White, Piece::Pawn)) => 'P', Some((Color::White, Piece::Knight)) => 'N', Some((Color::White, Piece::Bishop)) => 'B', Some((Color::White, Piece::Rook)) => 'R', Some((Color::White, Piece::Queen)) => 'Q', Some((Color::White, Piece::King)) => 'K', Some((Color::Black, Piece::Pawn)) => 'p', Some((Color::Black, Piece::Knight)) => 'n', Some((Color::Black, Piece::Bishop)) => 'b', Some((Color::Black, Piece::Rook)) => 'r', Some((Color::Black, Piece::Queen)) => 'q', Some((Color::Black, Piece::King)) => 'k', None => ' ', }; print!(" {} |", piece_char); } println!("\n  +---+---+---+---+---+---+---+---+"); }
        println!("    a   b   c   d   e   f   g   h");
        println!("Turn: {}", if self.white_to_move { "White" } else { "Black" });
        if let Some(ep_target) = self.en_passant_target { println!("EP Target: {}", format_square(ep_target)); }
        println!("Castling: {:?}", self.castling_rights);
        println!("------------------------------------");
    }
}

// --- Local helpers for FEN parsing ---
fn file_to_index(file: char) -> usize { file as usize - ('a' as usize) }
fn rank_to_index(rank: char) -> usize { (rank as usize) - ('0' as usize) - 1 }
