mod attack_tables;
mod eval;
mod make_unmake;
mod movegen;

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};

use crate::tt::{BoundType, TranspositionTable};
use crate::board::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};

// --- Helper functions ---
pub(crate) fn file_to_index(file: char) -> usize {
    file as usize - ('a' as usize)
}

pub(crate) fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('0' as usize) - 1
}

pub(crate) fn square_index(sq: Square) -> SquareIdx {
    SquareIdx((sq.0 * 8 + sq.1) as u8)
}

pub(crate) fn square_from_index(idx: SquareIdx) -> Square {
    let idx = idx.0 as usize;
    Square(idx / 8, idx % 8)
}

pub(crate) fn bit_for_square(sq: Square) -> Bitboard {
    Bitboard(1u64 << square_index(sq).0)
}

pub(crate) fn color_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}

pub(crate) fn piece_index(piece: Piece) -> usize {
    match piece {
        Piece::Pawn => 0,
        Piece::Knight => 1,
        Piece::Bishop => 2,
        Piece::Rook => 3,
        Piece::Queen => 4,
        Piece::King => 5,
    }
}

pub(crate) fn pop_lsb(bb: &mut Bitboard) -> SquareIdx {
    let idx = bb.0.trailing_zeros() as u8;
    bb.0 &= bb.0 - 1;
    SquareIdx(idx)
}

const CASTLE_WHITE_K: u8 = 1 << 0;
const CASTLE_WHITE_Q: u8 = 1 << 1;
const CASTLE_BLACK_K: u8 = 1 << 2;
const CASTLE_BLACK_Q: u8 = 1 << 3;

fn castle_bit(color: Color, side: char) -> u8 {
    match (color, side) {
        (Color::White, 'K') => CASTLE_WHITE_K,
        (Color::White, 'Q') => CASTLE_WHITE_Q,
        (Color::Black, 'K') => CASTLE_BLACK_K,
        (Color::Black, 'Q') => CASTLE_BLACK_Q,
        _ => 0,
    }
}

// --- Enums and Structs ---

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Color {
    White,
    Black,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SquareIdx(pub u8);

impl SquareIdx {
    pub(crate) fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Square(pub usize, pub usize); // (rank, file)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bitboard(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub is_castling: bool,
    pub is_en_passant: bool,
    pub promotion: Option<Piece>,
    pub captured_piece: Option<Piece>,
}

const MAX_MOVES: usize = 256;
const MAX_PLY: usize = 128;
const EMPTY_MOVE: Move = Move {
    from: Square(0, 0),
    to: Square(0, 0),
    is_castling: false,
    is_en_passant: false,
    promotion: None,
    captured_piece: None,
};

#[derive(Clone, Debug)]
pub struct MoveList {
    moves: [Move; MAX_MOVES],
    len: usize,
}

impl MoveList {
    fn new() -> Self {
        MoveList {
            moves: [EMPTY_MOVE; MAX_MOVES],
            len: 0,
        }
    }

    fn push(&mut self, mv: Move) {
        self.moves[self.len] = mv;
        self.len += 1;
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<'_, Move> {
        self.as_slice().iter()
    }
}

pub struct SearchState {
    tt: TranspositionTable,
    nodes: u64,
    seldepth: u32,
    generation: u16,
    total_nodes: u64,
    max_nodes: u64,
    killer_moves: [[Move; 2]; MAX_PLY],
    history: [i32; 4096],
    counter_moves: [[Move; 64]; 64],
    last_move: Move,
}

impl SearchState {
    pub fn new(tt_mb: usize) -> Self {
        SearchState {
            tt: TranspositionTable::new(tt_mb),
            nodes: 0,
            seldepth: 0,
            generation: 0,
            total_nodes: 0,
            max_nodes: 0,
            killer_moves: [[EMPTY_MOVE; 2]; MAX_PLY],
            history: [0; 4096],
            counter_moves: [[EMPTY_MOVE; 64]; 64],
            last_move: EMPTY_MOVE,
        }
    }

    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.nodes = 0;
        self.seldepth = 0;
        self.total_nodes = 0;
        self.last_move = EMPTY_MOVE;
    }

    pub fn set_max_nodes(&mut self, max_nodes: u64) {
        self.max_nodes = max_nodes;
    }

    pub fn hashfull_per_mille(&self) -> u32 {
        self.tt.hashfull_per_mille()
    }

    fn record_killer(&mut self, ply: usize, mv: Move) {
        if ply >= MAX_PLY {
            return;
        }
        if self.killer_moves[ply][0] != mv {
            self.killer_moves[ply][1] = self.killer_moves[ply][0];
            self.killer_moves[ply][0] = mv;
        }
    }

    fn is_killer(&self, ply: usize, mv: Move) -> bool {
        if ply >= MAX_PLY {
            return false;
        }
        self.killer_moves[ply][0] == mv || self.killer_moves[ply][1] == mv
    }

    fn add_history(&mut self, mv: Move, depth: u32) {
        let idx = move_history_index(mv);
        if idx < self.history.len() {
            self.history[idx] = self.history[idx].saturating_add((depth * depth) as i32);
        }
    }

    fn history_score(&self, mv: Move) -> i32 {
        let idx = move_history_index(mv);
        if idx < self.history.len() {
            self.history[idx]
        } else {
            0
        }
    }

    fn set_counter_move(&mut self, prev: Move, reply: Move) {
        let from = square_index(prev.from).0 as usize;
        let to = square_index(prev.to).0 as usize;
        self.counter_moves[from][to] = reply;
    }

    fn get_counter_move(&self, prev: Move) -> Option<Move> {
        let from = square_index(prev.from).0 as usize;
        let to = square_index(prev.to).0 as usize;
        let mv = self.counter_moves[from][to];
        if mv == EMPTY_MOVE {
            None
        } else {
            Some(mv)
        }
    }
}

pub struct SearchLimits {
    pub soft_time_ms: std::sync::Arc<AtomicU64>,
    pub hard_time_ms: std::sync::Arc<AtomicU64>,
    pub start_time: std::sync::Arc<Mutex<Instant>>,
    pub stop: std::sync::Arc<AtomicBool>,
}

#[derive(Clone, Debug)]
pub struct UnmakeInfo {
    captured_piece_info: Option<(Color, Piece)>,
    previous_en_passant_target: Option<Square>,
    previous_castling_rights: u8,
    previous_hash: u64, // Store previous hash for unmake
    previous_halfmove_clock: u32,
    made_hash: u64,
    previous_repetition_count: u32,
}

const KING_VALUE: i32 = 20000;
const MATE_SCORE: i32 = KING_VALUE * 10;

fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 300,
        Piece::Bishop => 300,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 10000, // Usually not used in MVV-LVA since king captures are illegal
    }
}

fn mvv_lva_score(m: &Move, board: &Board) -> i32 {
    if let Some(victim) = m.captured_piece {
        let attacker = board.piece_at(m.from).unwrap().1;
        let victim_value = piece_value(victim);
        let attacker_value = piece_value(attacker);
        victim_value * 10 - attacker_value // prioritize more valuable victims, less valuable attackers
    } else {
        0 // Non-captures get low priority
    }
}

fn move_history_index(m: Move) -> usize {
    let from = square_index(m.from).0 as usize;
    let to = square_index(m.to).0 as usize;
    from * 64 + to
}

#[derive(Clone, Debug)]
pub struct Board {
    pieces: [[Bitboard; 6]; 2],
    occupied: [Bitboard; 2],
    all_occupied: Bitboard,
    white_to_move: bool,
    en_passant_target: Option<Square>,
    castling_rights: u8, // bitmask
    hash: u64,           // Zobrist hash
    halfmove_clock: u32,
    repetition_counts: HashMap<u64, u32>,
}

impl Board {
    fn score_move(
        &self,
        state: &SearchState,
        m: &Move,
        ply: u32,
        hash_move: Option<Move>,
        counter_move: Option<Move>,
        pv_move: Option<Move>,
    ) -> i32 {
        const HASH_SCORE: i32 = 1_000_000;
        const PV_SCORE: i32 = 900_000;
        const CAPTURE_BASE: i32 = 500_000;
        const KILLER_SCORE: i32 = 400_000;
        const COUNTER_SCORE: i32 = 300_000;

        if let Some(hm) = hash_move {
            if *m == hm {
                return HASH_SCORE;
            }
        }

        if let Some(pv) = pv_move {
            if *m == pv {
                return PV_SCORE;
            }
        }

        if m.captured_piece.is_some() || m.is_en_passant {
            let see = self.see_capture(m);
            let mut score = CAPTURE_BASE + mvv_lva_score(m, self);
            if see < 0 {
                score -= 10_000;
            } else {
                score += see;
            }
            return score;
        }

        if state.is_killer(ply as usize, *m) {
            return KILLER_SCORE;
        }

        if let Some(cm) = counter_move {
            if *m == cm {
                return COUNTER_SCORE;
            }
        }

        state.history_score(*m)
    }

    fn is_bad_capture(&self, m: &Move) -> bool {
        if m.captured_piece.is_none() {
            return false;
        }
        self.see_capture(m) < 0
    }

    fn see_capture(&self, m: &Move) -> i32 {
        let captured = match m.captured_piece {
            Some(p) => p,
            None => return 0,
        };
        if m.is_en_passant {
            return 0;
        }
        let (moving_color, moving_piece) = match self.piece_at(m.from) {
            Some(info) => info,
            None => return 0,
        };
        let promotion_piece = m.promotion.unwrap_or(moving_piece);

        let mut pieces = self.pieces;
        let from_bb = 1u64 << square_index(m.from).0;
        let to_bb = 1u64 << square_index(m.to).0;
        let mover_idx = color_index(moving_color);
        let opp_idx = color_index(self.opponent_color(moving_color));

        pieces[opp_idx][piece_index(captured)].0 &= !to_bb;
        pieces[mover_idx][piece_index(moving_piece)].0 &= !from_bb;
        pieces[mover_idx][piece_index(promotion_piece)].0 |= to_bb;

        let mut occ = self.all_occupied.0;
        occ &= !from_bb;
        occ &= !to_bb;
        occ |= to_bb;

        let attackers_to = |color: Color, occ: u64, pieces: &[[Bitboard; 6]; 2]| -> u64 {
            let sq_idx = square_index(m.to).0 as usize;
            let c_idx = color_index(color);
            let pawns = if color == Color::White {
                pieces[c_idx][piece_index(Piece::Pawn)].0 & PAWN_ATTACKS[1][sq_idx]
            } else {
                pieces[c_idx][piece_index(Piece::Pawn)].0 & PAWN_ATTACKS[0][sq_idx]
            };
            let knights = pieces[c_idx][piece_index(Piece::Knight)].0 & KNIGHT_ATTACKS[sq_idx];
            let bishops = pieces[c_idx][piece_index(Piece::Bishop)].0
                & slider_attacks(sq_idx, occ, true);
            let rooks = pieces[c_idx][piece_index(Piece::Rook)].0
                & slider_attacks(sq_idx, occ, false);
            let queens = pieces[c_idx][piece_index(Piece::Queen)].0
                & (slider_attacks(sq_idx, occ, true) | slider_attacks(sq_idx, occ, false));
            let kings = pieces[c_idx][piece_index(Piece::King)].0 & KING_ATTACKS[sq_idx];
            pawns | knights | bishops | rooks | queens | kings
        };

        let mut gains = [0i32; 32];
        gains[0] = piece_value(captured);
        let mut depth = 0usize;
        let mut side = self.opponent_color(moving_color);

        loop {
            let attackers = attackers_to(side, occ, &pieces);
            if attackers == 0 {
                break;
            }

            let side_idx = color_index(side);
            let mut attacker_piece = None;
            let mut attacker_sq = 0u64;
            for piece in [
                Piece::Pawn,
                Piece::Knight,
                Piece::Bishop,
                Piece::Rook,
                Piece::Queen,
                Piece::King,
            ] {
                let bb = pieces[side_idx][piece_index(piece)].0 & attackers;
                if bb != 0 {
                    attacker_piece = Some(piece);
                    attacker_sq = bb & (!bb + 1);
                    break;
                }
            }
            let attacker_piece = match attacker_piece {
                Some(p) => p,
                None => break,
            };

            depth += 1;
            gains[depth] = piece_value(attacker_piece) - gains[depth - 1];
            if gains[depth].max(-gains[depth - 1]) < 0 {
                break;
            }

            pieces[side_idx][piece_index(attacker_piece)].0 &= !attacker_sq;
            pieces[side_idx][piece_index(attacker_piece)].0 |= to_bb;
            occ &= !attacker_sq;

            side = self.opponent_color(side);
        }

        while depth > 0 {
            let d = depth;
            gains[d - 1] = -std::cmp::max(-gains[d - 1], gains[d]);
            depth -= 1;
        }

        gains[0]
    }
    pub fn new() -> Self {
        let mut board = Board::empty();
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
            board.set_piece(Square(0, i), Color::White, *piece);
            board.set_piece(Square(7, i), Color::Black, *piece);
            board.set_piece(Square(1, i), Color::White, Piece::Pawn);
            board.set_piece(Square(6, i), Color::Black, Piece::Pawn);
        }

        board.castling_rights =
            CASTLE_WHITE_K | CASTLE_WHITE_Q | CASTLE_BLACK_K | CASTLE_BLACK_Q;
        board.white_to_move = true;
        board.hash = board.calculate_initial_hash();
        board.repetition_counts.insert(board.hash, 1);
        board
    }

    fn empty() -> Self {
        Board {
            pieces: [[Bitboard(0); 6]; 2],
            occupied: [Bitboard(0); 2],
            all_occupied: Bitboard(0),
            white_to_move: true,
            en_passant_target: None,
            castling_rights: 0,
            hash: 0,
            halfmove_clock: 0,
            repetition_counts: HashMap::new(),
        }
    }

    pub fn from_fen(fen: &str) -> Self {
        let mut board = Board::empty();
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
                    board.set_piece(Square(7 - rank_idx, file), color, piece);
                    file += 1;
                }
            }
        }
        board.white_to_move = match parts[1] {
            "w" => true,
            "b" => false,
            _ => panic!("Invalid color"),
        };
        for c in parts[2].chars() {
            match c {
                'K' => {
                    board.castling_rights |= CASTLE_WHITE_K;
                }
                'Q' => {
                    board.castling_rights |= CASTLE_WHITE_Q;
                }
                'k' => {
                    board.castling_rights |= CASTLE_BLACK_K;
                }
                'q' => {
                    board.castling_rights |= CASTLE_BLACK_Q;
                }
                '-' => {}
                _ => panic!("Invalid castle"),
            }
        }
        board.en_passant_target = if parts[3] != "-" {
            let chars: Vec<char> = parts[3].chars().collect();
            if chars.len() == 2 {
                Some(Square(rank_to_index(chars[1]), file_to_index(chars[0])))
            } else {
                None
            }
        } else {
            None
        };

        if parts.len() >= 5 {
            board.halfmove_clock = parts[4].parse().unwrap_or(0);
        }

        board.hash = board.calculate_initial_hash();
        board.repetition_counts.insert(board.hash, 1);
        board
    }

    pub fn hash(&self) -> u64 {
        self.hash
    }

    pub fn white_to_move(&self) -> bool {
        self.white_to_move
    }

    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove_clock
    }

    pub fn is_draw(&self) -> bool {
        if self.halfmove_clock >= 100 {
            return true;
        }
        self.repetition_counts.get(&self.hash).copied().unwrap_or(0) >= 3
    }

    pub fn is_theoretical_draw(&self) -> bool {
        self.is_draw() || self.is_insufficient_material()
    }

    fn is_insufficient_material(&self) -> bool {
        let white = color_index(Color::White);
        let black = color_index(Color::Black);

        let pawns = self.pieces[white][piece_index(Piece::Pawn)].0
            | self.pieces[black][piece_index(Piece::Pawn)].0;
        let rooks = self.pieces[white][piece_index(Piece::Rook)].0
            | self.pieces[black][piece_index(Piece::Rook)].0;
        let queens = self.pieces[white][piece_index(Piece::Queen)].0
            | self.pieces[black][piece_index(Piece::Queen)].0;

        if pawns != 0 || rooks != 0 || queens != 0 {
            return false;
        }

        let white_knights =
            self.pieces[white][piece_index(Piece::Knight)].0.count_ones();
        let black_knights =
            self.pieces[black][piece_index(Piece::Knight)].0.count_ones();
        let white_bishops =
            self.pieces[white][piece_index(Piece::Bishop)].0.count_ones();
        let black_bishops =
            self.pieces[black][piece_index(Piece::Bishop)].0.count_ones();

        let total_minors = white_knights + black_knights + white_bishops + black_bishops;

        if total_minors == 0 || total_minors == 1 {
            return true;
        }

        let total_knights = white_knights + black_knights;
        let total_bishops = white_bishops + black_bishops;

        let bishops_all_same_color = |mut bishops: Bitboard| -> bool {
            let mut bishop_color: Option<u8> = None;
            while bishops.0 != 0 {
                let sq = pop_lsb(&mut bishops);
                let square = square_from_index(sq);
                let color = ((square.0 + square.1) % 2) as u8;
                match bishop_color {
                    Some(existing) if existing != color => return false,
                    Some(_) => {}
                    None => bishop_color = Some(color),
                }
            }
            true
        };

        if total_minors == 2 {
            if total_bishops == 0 {
                return true; // Knights only.
            }
            if total_knights == 0 {
                if white_bishops == 1 && black_bishops == 1 {
                    return true; // Bishop vs bishop is always insufficient.
                }
                let bishops = Bitboard(
                    self.pieces[white][piece_index(Piece::Bishop)].0
                        | self.pieces[black][piece_index(Piece::Bishop)].0,
                );
                return bishops_all_same_color(bishops);
            }

            let white_minors = white_knights + white_bishops;
            let black_minors = black_knights + black_bishops;
            return white_minors == 1 && black_minors == 1;
        }

        if total_knights == 0 {
            let bishops = Bitboard(
                self.pieces[white][piece_index(Piece::Bishop)].0
                    | self.pieces[black][piece_index(Piece::Bishop)].0,
            );
            return bishops_all_same_color(bishops);
        }

        false
    }

    pub fn print(&self) {
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let piece_char = match self.piece_at(Square(rank, file)) {
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
        println!("Turn: {}", if self.white_to_move { "White" } else { "Black" });
        if let Some(ep_target) = self.en_passant_target {
            println!("EP Target: {}", format_square(ep_target));
        }
        println!("Castling mask: {:#06b}", self.castling_rights);
        println!("------------------------------------");
    }

    pub fn debug_bitboards(&self) {
        let colors = [Color::White, Color::Black];
        let pieces = [
            (Piece::Pawn, "P"),
            (Piece::Knight, "N"),
            (Piece::Bishop, "B"),
            (Piece::Rook, "R"),
            (Piece::Queen, "Q"),
            (Piece::King, "K"),
        ];

        println!(
            "Side to move: {}",
            if self.white_to_move { "White" } else { "Black" }
        );
        println!("Castling mask: {:#06b}", self.castling_rights);
        if let Some(ep_target) = self.en_passant_target {
            println!("EP Target: {}", format_square(ep_target));
        }
        println!("All occupied: {:#018x}", self.all_occupied.0);

        for color in colors {
            let label = if color == Color::White { "White" } else { "Black" };
            for (piece, name) in pieces {
                let bb = self.pieces[color_index(color)][piece_index(piece)].0;
                println!("{} {}: {:#018x}", label, name, bb);
            }
        }
        println!("------------------------------------");
    }

    pub fn print_bitboard_grid(&self, label: &str, bb: Bitboard) {
        println!("{} {:#018x}", label, bb.0);
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let idx = (rank * 8 + file) as u8;
                let ch = if (bb.0 >> idx) & 1 == 1 { '1' } else { '.' };
                print!(" {} |", ch);
            }
            println!("\n  +---+---+---+---+---+---+---+---+");
        }
        println!("    a   b   c   d   e   f   g   h");
        println!("------------------------------------");
    }
}

impl Board {
    fn negamax(
        &mut self,
        state: &mut SearchState,
        depth: u32,
        ply: u32,
        stop: &AtomicBool,
        mut alpha: i32,
        mut beta: i32,
    ) -> i32 {
        const NULL_REDUCTION: u32 = 2;
        const NULL_MIN_DEPTH: u32 = 3;
        const NULL_VERIFICATION_DEPTH: u32 = 6;
        const FUTILITY_MARGIN: i32 = 150;
        const RAZOR_MARGIN: i32 = 250;
        const LMR_MIN_DEPTH: u32 = 3;
        const LMR_MIN_MOVE: usize = 3;
        const LMR_REDUCTION: u32 = 1;
        const LMP_MIN_DEPTH: u32 = 3;
        const LMP_MOVE_LIMIT: usize = 8;
        const IIR_MIN_DEPTH: u32 = 6;
        const SINGULAR_MARGIN: i32 = 50;
        const RFP_MARGIN: i32 = 100;
        const STATIC_NULL_MARGIN: i32 = 120;

        if stop.load(Ordering::Relaxed) {
            return 0;
        }
        state.nodes += 1;
        state.total_nodes += 1;
        if state.max_nodes > 0 && state.total_nodes >= state.max_nodes {
            stop.store(true, Ordering::Relaxed);
            return 0;
        }
        if ply > state.seldepth {
            state.seldepth = ply;
        }
        if self.is_draw() {
            return 0;
        }

        let original_alpha = alpha;
        let current_hash = self.hash;

        let mut hash_move: Option<Move> = None;
        let mut tt_eval: Option<i32> = None;
        if let Some(entry) = state.tt.probe(current_hash) {
            if entry.depth >= depth {
                let score = adjust_mate_score_for_retrieve(entry.score, ply);
                match entry.bound_type {
                    BoundType::Exact => return score,
                    BoundType::LowerBound => alpha = alpha.max(score),
                    BoundType::UpperBound => beta = beta.min(score),
                }
                if alpha >= beta {
                    return score;
                }
            }
            hash_move = entry.best_move.clone();
            tt_eval = Some(entry.eval);
        }

        let in_check = self.is_in_check(self.current_color());

        let mut depth = depth;
        if depth >= IIR_MIN_DEPTH {
            depth = depth.saturating_sub(1);
        }
        if in_check {
            depth = depth.saturating_add(1);
        }

        if depth == 0 {
            return self.quiesce(state, ply, stop, alpha, beta);
        }

        if !in_check && depth >= NULL_MIN_DEPTH && !self.is_theoretical_draw() {
            self.white_to_move = !self.white_to_move;
            let score =
                -self.negamax(state, depth - 1 - NULL_REDUCTION, ply + 1, stop, -beta, -beta + 1);
            self.white_to_move = !self.white_to_move;
            if score >= beta {
                if depth >= NULL_VERIFICATION_DEPTH {
                    let verify =
                        self.negamax(state, depth - 1, ply + 1, stop, beta - 1, beta);
                    if verify >= beta {
                        return score;
                    }
                } else {
                    return score;
                }
            }
        }

        if !in_check && depth <= 2 {
            let stand_pat = self.evaluate();
            if stand_pat + RAZOR_MARGIN < alpha {
                return self.quiesce(state, ply, stop, alpha, beta);
            }
        }

        let mut legal_moves = self.generate_moves();
        let counter_move = state.get_counter_move(state.last_move);
        legal_moves.as_mut_slice().sort_by_key(|m| {
            -self.score_move(state, m, ply, hash_move, counter_move, None)
        });

        if legal_moves.is_empty() {
            let current_color = self.current_color();
            return if self.is_in_check(current_color) {
                -(MATE_SCORE - (100 - depth as i32))
            } else {
                0
            };
        }

        if let Some(hm) = &hash_move {
            if let Some(pos) = legal_moves.as_slice().iter().position(|m| m == hm) {
                legal_moves.as_mut_slice().swap(0, pos);
            }
        }

        let mut best_score = -MATE_SCORE * 2;
        let mut best_move_found: Option<Move> = None;
        let singular_target = if let Some(hm) = &hash_move {
            Some((*hm, alpha + SINGULAR_MARGIN))
        } else {
            None
        };

        let eval_at_node = self.evaluate();
        let stand_pat = if in_check {
            alpha
        } else {
            tt_eval.unwrap_or(eval_at_node)
        };

        if !in_check && depth <= 4 && stand_pat - RFP_MARGIN * depth as i32 >= beta {
            return stand_pat;
        }

        if !in_check && depth <= 3 && stand_pat + STATIC_NULL_MARGIN * depth as i32 <= alpha {
            return stand_pat;
        }

        for (i, m) in legal_moves.iter().enumerate() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if depth >= LMP_MIN_DEPTH
                && i >= LMP_MOVE_LIMIT
                && m.captured_piece.is_none()
                && m.promotion.is_none()
                && !in_check
            {
                break;
            }
            if !in_check
                && depth <= 2
                && m.captured_piece.is_none()
                && m.promotion.is_none()
                && stand_pat + FUTILITY_MARGIN <= alpha
            {
                continue;
            }
            let info = self.make_move(&m);
            let prev_last = state.last_move;
            state.last_move = *m;
            let mut new_depth = depth - 1;
            if !in_check
                && depth >= LMR_MIN_DEPTH
                && i >= LMR_MIN_MOVE
                && m.captured_piece.is_none()
                && m.promotion.is_none()
            {
                new_depth = new_depth.saturating_sub(LMR_REDUCTION);
            }

            let score = if i == 0 {
                if let Some((hm, target)) = singular_target {
                    if *m == hm && depth >= 6 {
                        let mut reduced = new_depth.saturating_sub(2);
                        let sing_score =
                            -self.negamax(state, reduced, ply + 1, stop, -target, -alpha);
                        if sing_score > -target {
                            reduced = new_depth;
                        }
                        -self.negamax(state, reduced, ply + 1, stop, -beta, -alpha)
                    } else {
                        -self.negamax(state, new_depth, ply + 1, stop, -beta, -alpha)
                    }
                } else {
                    -self.negamax(state, new_depth, ply + 1, stop, -beta, -alpha)
                }
            } else {
                let mut score =
                    -self.negamax(state, new_depth, ply + 1, stop, -alpha - 1, -alpha);
                if score > alpha && score < beta {
                    score = -self.negamax(state, new_depth, ply + 1, stop, -beta, -alpha);
                }
                score
            };
            state.last_move = prev_last;
            self.unmake_move(&m, info);

            if score > best_score {
                best_score = score;
                best_move_found = Some(m.clone());
            }

            alpha = alpha.max(best_score);

            if alpha >= beta {
                let is_quiet =
                    m.captured_piece.is_none() && m.promotion.is_none() && !m.is_en_passant;
                if is_quiet {
                    state.record_killer(ply as usize, *m);
                    state.add_history(*m, depth);
                    state.set_counter_move(state.last_move, *m);
                }
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

        state.tt.store(
            current_hash,
            depth,
            adjust_mate_score_for_store(best_score, ply),
            bound_type,
            best_move_found,
            state.generation,
            eval_at_node,
        );

        best_score
    }

    fn quiesce(
        &mut self,
        state: &mut SearchState,
        ply: u32,
        stop: &AtomicBool,
        mut alpha: i32,
        beta: i32,
    ) -> i32 {
        const DELTA_MARGIN: i32 = 200;
        if stop.load(Ordering::Relaxed) {
            return 0;
        }
        state.nodes += 1;
        state.total_nodes += 1;
        if state.max_nodes > 0 && state.total_nodes >= state.max_nodes {
            stop.store(true, Ordering::Relaxed);
            return 0;
        }
        if ply > state.seldepth {
            state.seldepth = ply;
        }
        if self.is_draw() {
            return 0;
        }

        let stand_pat_score = self.evaluate();

        if stand_pat_score >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat_score);

        let mut tactical_moves = self.generate_tactical_moves();
        let checking_moves = self.generate_checking_moves();
        for m in checking_moves.iter() {
            if m.captured_piece.is_none() && !m.is_en_passant {
                tactical_moves.push(*m);
            }
        }
        tactical_moves
            .as_mut_slice()
            .sort_by_key(|m| -mvv_lva_score(m, self));

        let mut best_score = stand_pat_score;

        for m in tactical_moves.iter() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let bad_capture = self.is_bad_capture(m);
            let capture_value = m.captured_piece.map(piece_value).unwrap_or(0);
            let info = self.make_move(m);
            let gives_check = self.is_in_check(self.current_color());
            if !gives_check
                && stand_pat_score + capture_value + DELTA_MARGIN < alpha
                && !m.is_en_passant
            {
                self.unmake_move(m, info);
                continue;
            }
            if bad_capture && !gives_check {
                self.unmake_move(m, info);
                continue;
            }
            let score = -self.quiesce(state, ply + 1, stop, -beta, -alpha);
            self.unmake_move(m, info);

            best_score = best_score.max(score);
            alpha = alpha.max(best_score);

            if alpha >= beta {
                break;
            }
        }

        alpha
    }
}

pub fn format_square(sq: Square) -> String {
    format!("{}{}", (sq.1 as u8 + b'a') as char, sq.0 + 1)
}

fn format_uci_move_for_info(mv: &Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => '?',
        });
    }
    s
}

fn format_score_for_info(score: i32) -> String {
    if score.abs() >= MATE_SCORE - 100 {
        let mate_plies = (MATE_SCORE - score.abs()).max(0) as u32;
        let mate_moves = (mate_plies + 1) / 2;
        let signed = if score > 0 {
            mate_moves as i32
        } else {
            -(mate_moves as i32)
        };
        format!("mate {}", signed)
    } else {
        format!("cp {}", score)
    }
}

fn adjust_mate_score_for_store(score: i32, ply: u32) -> i32 {
    if score.abs() >= MATE_SCORE - 100 {
        if score > 0 {
            score + ply as i32
        } else {
            score - ply as i32
        }
    } else {
        score
    }
}

fn adjust_mate_score_for_retrieve(score: i32, ply: u32) -> i32 {
    if score.abs() >= MATE_SCORE - 100 {
        if score > 0 {
            score - ply as i32
        } else {
            score + ply as i32
        }
    } else {
        score
    }
}

fn format_pv(moves: &[Move]) -> String {
    moves
        .iter()
        .map(format_uci_move_for_info)
        .collect::<Vec<String>>()
        .join(" ")
}

fn build_pv(board: &mut Board, state: &SearchState, max_len: usize) -> Vec<Move> {
    let mut pv = Vec::new();
    let mut history: Vec<(Move, UnmakeInfo)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for _ in 0..max_len {
        let hash = board.hash();
        if !seen.insert(hash) {
            break;
        }
        let entry = match state.tt.probe(hash) {
            Some(e) => e,
            None => break,
        };
        let mv = match entry.best_move {
            Some(m) => m,
            None => break,
        };
        let legal_moves = board.generate_moves();
        if !legal_moves.as_slice().iter().any(|m| *m == mv) {
            break;
        }
        let info = board.make_move(&mv);
        history.push((mv, info));
        pv.push(mv);
    }

    while let Some((mv, info)) = history.pop() {
        board.unmake_move(&mv, info);
    }

    pv
}

pub fn find_best_move(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    stop: &AtomicBool,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut last_score = 0;

    let legal_moves = board.generate_moves();
    if legal_moves.is_empty() {
        return None;
    }
    if legal_moves.len() == 1 {
        return Some(legal_moves.as_slice()[0]);
    }
    let mut root_moves = legal_moves.clone();

    for depth in 1..=max_depth {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let window = if depth <= 2 { MATE_SCORE } else { 50 };
        let mut alpha = last_score - window;
        let mut beta = last_score + window;

        let (mut current_best_score, mut current_best_move) =
            root_search(board, state, depth, alpha, beta, stop, &mut root_moves, best_move);

        if current_best_score <= alpha || current_best_score >= beta {
            alpha = -MATE_SCORE * 2;
            beta = MATE_SCORE * 2;
            let result =
                root_search(board, state, depth, alpha, beta, stop, &mut root_moves, best_move);
            current_best_score = result.0;
            current_best_move = result.1;
        }

        if let Some(mv) = current_best_move {
            best_move = Some(mv);
            last_score = current_best_score;

            if let Some(pos) = root_moves.as_slice().iter().position(|m| *m == mv) {
                root_moves.as_mut_slice().swap(0, pos);
            }
        }
    }

    best_move
}

pub fn find_best_move_with_time(
    board: &mut Board,
    state: &mut SearchState,
    limits: SearchLimits,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut depth = 1;
    let mut last_depth_time = Duration::from_millis(1);
    let mut last_score = 0;

    const SAFETY_MARGIN: Duration = Duration::from_millis(5);
    const TIME_GROWTH_FACTOR: f32 = 2.0;

    while !limits.stop.load(Ordering::Relaxed) {
        let soft_time_ms = limits.soft_time_ms.load(Ordering::Relaxed);
        let hard_time_ms = limits.hard_time_ms.load(Ordering::Relaxed);
        let infinite = hard_time_ms == u64::MAX;
        let soft_time = Duration::from_millis(soft_time_ms);
        let hard_time = Duration::from_millis(hard_time_ms);
        let start_time = *limits.start_time.lock().unwrap();
        let elapsed = start_time.elapsed();
        if !infinite && elapsed + SAFETY_MARGIN >= hard_time {
            break;
        }
        let time_remaining = soft_time.checked_sub(elapsed).unwrap_or_default();

        let estimated_next_time = last_depth_time.mul_f32(TIME_GROWTH_FACTOR);
        if !infinite && estimated_next_time + SAFETY_MARGIN > time_remaining {
            break;
        }

        let depth_start = Instant::now();
        state.nodes = 0;
        state.seldepth = 0;

        let mut legal_moves = board.generate_moves();

        if legal_moves.is_empty() {
            return None;
        }

        if legal_moves.len() == 1 {
            return Some(legal_moves.as_slice()[0]);
        }

        let window = if depth <= 2 { MATE_SCORE } else { 50 };
        let mut alpha = last_score - window;
        let mut beta = last_score + window;

        let (mut best_score, mut new_best_move) =
            root_search(board, state, depth, alpha, beta, &limits.stop, &mut legal_moves, best_move);

        if best_score <= alpha || best_score >= beta {
            alpha = -MATE_SCORE * 2;
            beta = MATE_SCORE * 2;
            let result =
                root_search(board, state, depth, alpha, beta, &limits.stop, &mut legal_moves, best_move);
            best_score = result.0;
            new_best_move = result.1;
        }

        if !limits.stop.load(Ordering::Relaxed) {
            best_move = new_best_move;
            last_depth_time = depth_start.elapsed();
            last_score = best_score;
            if let Some(mv) = best_move {
                let start_time = *limits.start_time.lock().unwrap();
                let elapsed = start_time.elapsed();
                let time_ms = elapsed.as_millis().max(1) as u64;
                let nps = (state.nodes * 1000) / time_ms;
                let pv_moves = build_pv(board, state, 16);
                let pv = if pv_moves.is_empty() {
                    format_uci_move_for_info(&mv)
                } else {
                    format_pv(&pv_moves)
                };
                let score = format_score_for_info(best_score);
                println!(
                    "info depth {} seldepth {} score {} nodes {} nps {} hashfull {} time {} pv {}",
                    depth,
                    state.seldepth,
                    score,
                    state.nodes,
                    nps,
                    state.hashfull_per_mille(),
                    time_ms,
                    pv
                );
                if !infinite && elapsed + SAFETY_MARGIN >= soft_time {
                    break;
                }
            }
            depth += 1;
        }
    }

    if best_move.is_none() {
        let legal_moves = board.generate_moves();
        if !legal_moves.is_empty() {
            return Some(legal_moves.as_slice()[0]);
        }
    }

    best_move
}

fn root_search(
    board: &mut Board,
    state: &mut SearchState,
    depth: u32,
    mut alpha: i32,
    beta: i32,
    stop: &AtomicBool,
    root_moves: &mut MoveList,
    pv_move: Option<Move>,
) -> (i32, Option<Move>) {
    if stop.load(Ordering::Relaxed) {
        return (0, None);
    }

    let hash_move = state.tt.probe(board.hash()).and_then(|e| e.best_move);
    root_moves
        .as_mut_slice()
        .sort_by_key(|m| -board.score_move(state, m, 0, hash_move, None, pv_move));
    if let Some(entry) = state.tt.probe(board.hash()) {
        if let Some(hm) = &entry.best_move {
            if let Some(pos) = root_moves.as_slice().iter().position(|m| m == hm) {
                root_moves.as_mut_slice().swap(0, pos);
            }
        }
    }

    let mut best_score = -MATE_SCORE * 2;
    let mut best_move = if root_moves.is_empty() {
        None
    } else {
        Some(root_moves.as_slice()[0])
    };

    for m in root_moves.iter() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let info = board.make_move(m);
        let score = -board.negamax(state, depth - 1, 1, stop, -beta, -alpha);
        board.unmake_move(m, info);

        if score > best_score {
            best_score = score;
            best_move = Some(*m);
        }

        alpha = alpha.max(best_score);
        if alpha >= beta {
            break;
        }
    }

    (best_score, best_move)
}

#[cfg(test)]
mod perft_tests {
    use super::*;

    struct TestPosition {
        name: &'static str,
        fen: &'static str,
        depths: &'static [(usize, u64)],
    }

    const TEST_POSITIONS: &[TestPosition] = &[
        TestPosition {
            name: "Initial Position",
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            depths: &[
                (1, 20),
                (2, 400),
                (3, 8902),
                (4, 197281),
                (5, 4865609),
            ],
        },
        TestPosition {
            name: "Kiwipete",
            fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            depths: &[(1, 48), (2, 2039), (3, 97862), (4, 4085603)],
        },
        TestPosition {
            name: "Position 3",
            fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            depths: &[(1, 14), (2, 191), (3, 2812), (4, 43238), (5, 674624)],
        },
        TestPosition {
            name: "Position 4",
            fen: "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            depths: &[(1, 6), (2, 264), (3, 9467), (4, 422333)],
        },
        TestPosition {
            name: "Position 5",
            fen: "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            depths: &[(1, 44), (2, 1486), (3, 62379), (4, 2103487)],
        },
        TestPosition {
            name: "Position 6 (Win at Chess)",
            fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            depths: &[(1, 46), (2, 2079), (3, 89890)],
        },
        TestPosition {
            name: "En Passant Capture",
            fen: "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
            depths: &[(1, 31), (2, 707), (3, 21637)],
        },
        TestPosition {
            name: "Promotion",
            fen: "n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1",
            depths: &[(1, 24), (2, 496), (3, 9483)],
        },
        TestPosition {
            name: "Castling",
            fen: "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
            depths: &[(1, 26), (2, 568), (3, 13744)],
        },
    ];

    #[test]
    fn test_all_perft_positions() {
        for position in TEST_POSITIONS {
            println!("Testing position: {}", position.name);
            println!("FEN: {}", position.fen);

            let mut board = Board::from_fen(position.fen);

            for &(depth, expected) in position.depths {
                let start = Instant::now();
                let nodes = board.perft(depth);
                let duration = start.elapsed();

                println!("  Depth {}: {} nodes in {:?}", depth, nodes, duration);

                assert_eq!(
                    nodes, expected,
                    "Perft failed for position '{}' at depth {}. Expected: {}, Got: {}",
                    position.name, depth, expected, nodes
                );
            }
            println!("------------------------------");
        }
    }
}

#[cfg(test)]
mod draw_tests {
    use super::*;
    use crate::uci::parse_uci_move;

    fn find_move(board: &mut Board, from: Square, to: Square, promotion: Option<Piece>) -> Move {
        for m in board.generate_moves().iter() {
            if m.from == from && m.to == to && m.promotion == promotion {
                return *m;
            }
        }
        panic!("Expected move not found");
    }

    fn apply_uci(board: &mut Board, uci: &str) {
        let mv = parse_uci_move(board, uci).expect("uci move not legal");
        board.make_move(&mv);
    }

    #[test]
    fn test_fen_halfmove_parsing() {
        let board = Board::from_fen("8/8/8/8/8/8/8/K1k5 w - - 57 1");
        assert_eq!(board.halfmove_clock(), 57);
    }

    #[test]
    fn test_fifty_move_rule_draw() {
        let board = Board::from_fen("8/8/8/8/8/8/8/K1k5 w - - 100 1");
        assert!(board.is_draw());
        assert!(board.is_theoretical_draw());
    }

    #[test]
    fn test_halfmove_resets_on_pawn_move() {
        let mut board = Board::from_fen("8/8/8/8/8/8/4P3/K1k5 w - - 99 1");
        let mv = find_move(&mut board, Square(1, 4), Square(3, 4), None);
        board.make_move(&mv);
        assert_eq!(board.halfmove_clock(), 0);
        assert!(!board.is_draw());
        assert!(!board.is_theoretical_draw());
    }

    #[test]
    fn test_threefold_repetition() {
        let mut board = Board::new();
        for _ in 0..2 {
            apply_uci(&mut board, "g1f3");
            apply_uci(&mut board, "g8f6");
            apply_uci(&mut board, "f3g1");
            apply_uci(&mut board, "f6g8");
        }
        assert!(board.is_draw());
        assert!(board.is_theoretical_draw());
    }

    #[test]
    fn test_insufficient_material_draw() {
        let board = Board::from_fen("8/8/8/8/8/8/6N1/K1k5 w - - 0 1");
        assert!(!board.is_draw());
        assert!(board.is_theoretical_draw());
    }

    #[test]
    fn test_unmake_restores_state() {
        let mut board = Board::new();
        let original_hash = board.hash();
        let original_castling = board.castling_rights;
        let original_ep = board.en_passant_target;
        let original_halfmove = board.halfmove_clock();
        let original_rep = board
            .repetition_counts
            .get(&original_hash)
            .copied()
            .unwrap_or(0);

        let mv = find_move(&mut board, Square(1, 4), Square(3, 4), None);
        let info = board.make_move(&mv);
        board.unmake_move(&mv, info);

        assert_eq!(board.hash(), original_hash);
        assert_eq!(board.castling_rights, original_castling);
        assert_eq!(board.en_passant_target, original_ep);
        assert_eq!(board.halfmove_clock(), original_halfmove);
        assert_eq!(
            board.repetition_counts.get(&original_hash).copied().unwrap_or(0),
            original_rep
        );
    }

    #[test]
    fn test_draw_in_search() {
        let mut board = Board::from_fen("8/8/8/8/8/8/8/K1k5 w - - 100 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let score = board.negamax(&mut state, 1, 0, &stop, -1000, 1000);
        assert_eq!(score, 0);
    }
}

#[cfg(test)]
mod engine_tests {
    use super::*;
    use rand::prelude::*;

    fn find_move(board: &mut Board, from: Square, to: Square, promotion: Option<Piece>) -> Move {
        for m in board.generate_moves().iter() {
            if m.from == from && m.to == to && m.promotion == promotion {
                return *m;
            }
        }
        panic!("Expected move not found");
    }

    #[test]
    fn test_en_passant_make_unmake() {
        let mut board = Board::from_fen("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3");
        let original_hash = board.hash();
        let original_ep = board.en_passant_target;
        let mv = find_move(&mut board, Square(4, 4), Square(5, 5), None);
        let info = board.make_move(&mv);
        board.unmake_move(&mv, info);
        assert_eq!(board.hash(), original_hash);
        assert_eq!(board.en_passant_target, original_ep);
    }

    #[test]
    fn test_promotion_make_unmake() {
        let mut board = Board::from_fen("8/P7/8/8/8/8/8/K1k5 w - - 0 1");
        let original_hash = board.hash();
        let mv = find_move(&mut board, Square(6, 0), Square(7, 0), Some(Piece::Queen));
        let info = board.make_move(&mv);
        board.unmake_move(&mv, info);
        assert_eq!(board.hash(), original_hash);
        assert_eq!(board.piece_at(Square(6, 0)), Some((Color::White, Piece::Pawn)));
    }

    #[test]
    fn test_hash_matches_recompute_after_random_moves() {
        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(0xC0FFEE);
        let mut history: Vec<(Move, UnmakeInfo)> = Vec::new();

        for _ in 0..50 {
            let moves = board.generate_moves();
            if moves.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..moves.len());
            let mv = moves.as_slice()[idx];
            let info = board.make_move(&mv);
            history.push((mv, info));

            let recomputed = board.calculate_initial_hash();
            assert_eq!(board.hash(), recomputed);
        }

        while let Some((mv, info)) = history.pop() {
            board.unmake_move(&mv, info);
            let recomputed = board.calculate_initial_hash();
            assert_eq!(board.hash(), recomputed);
        }
    }

    #[test]
    fn test_random_playout_round_trip_state() {
        let mut board = Board::new();
        let initial_hash = board.hash();
        let initial_halfmove = board.halfmove_clock();
        let initial_castling = board.castling_rights;
        let initial_ep = board.en_passant_target;
        let initial_rep = board
            .repetition_counts
            .get(&initial_hash)
            .copied()
            .unwrap_or(0);

        let mut rng = StdRng::seed_from_u64(0x5EED);
        let mut history: Vec<(Move, UnmakeInfo)> = Vec::new();

        for _ in 0..200 {
            let moves = board.generate_moves();
            if moves.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..moves.len());
            let mv = moves.as_slice()[idx];
            let info = board.make_move(&mv);
            history.push((mv, info));
            let recomputed = board.calculate_initial_hash();
            assert_eq!(board.hash(), recomputed);
        }

        while let Some((mv, info)) = history.pop() {
            board.unmake_move(&mv, info);
        }

        assert_eq!(board.hash(), initial_hash);
        assert_eq!(board.halfmove_clock(), initial_halfmove);
        assert_eq!(board.castling_rights, initial_castling);
        assert_eq!(board.en_passant_target, initial_ep);
        assert_eq!(
            board.repetition_counts.get(&initial_hash).copied().unwrap_or(0),
            initial_rep
        );
    }
}
