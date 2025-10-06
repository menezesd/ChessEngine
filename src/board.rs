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
        // ...existing make_move code...
        unimplemented!()
    }

    pub fn unmake_move(&mut self, m: &Move, info: UnmakeInfo) {
        // ...existing unmake_move code...
        unimplemented!()
    }

    pub fn generate_moves(&mut self) -> Vec<Move> {
        // ...existing generate_moves code...
        unimplemented!()
    }

    // ... Other Board methods omitted for brevity ...
}
