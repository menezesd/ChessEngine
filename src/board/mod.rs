mod attack_tables;
mod eval;
mod make_unmake;
mod movegen;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::tt::{BoundType, TranspositionTable};

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
}

impl SearchState {
    pub fn new(tt_mb: usize) -> Self {
        SearchState {
            tt: TranspositionTable::new(tt_mb),
        }
    }
}

pub struct SearchLimits {
    pub max_time: Duration,
    pub start_time: Instant,
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
        mut alpha: i32,
        mut beta: i32,
    ) -> i32 {
        if self.is_draw() {
            return 0;
        }

        let original_alpha = alpha;
        let current_hash = self.hash;

        let mut hash_move: Option<Move> = None;
        if let Some(entry) = state.tt.probe(current_hash) {
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
            return self.quiesce(state, alpha, beta);
        }

        let mut legal_moves = self.generate_moves();
        legal_moves.as_mut_slice().sort_by_key(|m| -mvv_lva_score(m, self));

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

        for (i, m) in legal_moves.iter().enumerate() {
            let info = self.make_move(&m);
            let score = if i == 0 {
                -self.negamax(state, depth - 1, -beta, -alpha)
            } else {
                let mut score = -self.negamax(state, depth - 1, -alpha - 1, -alpha);
                if score > alpha && score < beta {
                    score = -self.negamax(state, depth - 1, -beta, -alpha);
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

        state
            .tt
            .store(current_hash, depth, best_score, bound_type, best_move_found);

        best_score
    }

    fn quiesce(&mut self, state: &mut SearchState, mut alpha: i32, beta: i32) -> i32 {
        if self.is_draw() {
            return 0;
        }

        let stand_pat_score = self.evaluate();

        if stand_pat_score >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat_score);

        let mut tactical_moves = self.generate_tactical_moves();
        tactical_moves.as_mut_slice().sort_by_key(|m| -mvv_lva_score(m, self));

        let mut best_score = stand_pat_score;

        for m in tactical_moves.iter() {
            let info = self.make_move(m);
            let score = -self.quiesce(state, -beta, -alpha);
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

pub fn find_best_move(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;

    let legal_moves = board.generate_moves();
    if legal_moves.is_empty() {
        return None;
    }
    if legal_moves.len() == 1 {
        return Some(legal_moves.as_slice()[0]);
    }
    let mut root_moves = legal_moves.clone();

    for depth in 1..=max_depth {
        let mut alpha = -MATE_SCORE * 2;
        let beta = MATE_SCORE * 2;
        let mut current_best_score = -MATE_SCORE * 2;
        let mut current_best_move: Option<Move> = None;

        if let Some(entry) = state.tt.probe(board.hash()) {
            if let Some(hm) = &entry.best_move {
                if let Some(pos) = root_moves.as_slice().iter().position(|m| m == hm) {
                    root_moves.as_mut_slice().swap(0, pos);
                }
            }
        }

        for m in root_moves.iter() {
            let info = board.make_move(m);
            let score = -board.negamax(state, depth - 1, -beta, -alpha);
            board.unmake_move(m, info);

            if score > current_best_score {
                current_best_score = score;
                current_best_move = Some(*m);
            }

            alpha = alpha.max(current_best_score);
        }

        if let Some(mv) = current_best_move {
            best_move = Some(mv);

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

    const SAFETY_MARGIN: Duration = Duration::from_millis(5);
    const TIME_GROWTH_FACTOR: f32 = 2.0;

    while limits.start_time.elapsed() + SAFETY_MARGIN < limits.max_time {
        let elapsed = limits.start_time.elapsed();
        let time_remaining = limits.max_time.checked_sub(elapsed).unwrap_or_default();

        let estimated_next_time = last_depth_time.mul_f32(TIME_GROWTH_FACTOR);
        if estimated_next_time + SAFETY_MARGIN > time_remaining {
            break;
        }

        let depth_start = Instant::now();

        let mut alpha = -MATE_SCORE * 2;
        let beta = MATE_SCORE * 2;
        let mut best_score = -MATE_SCORE * 2;
        let mut legal_moves = board.generate_moves();

        if legal_moves.is_empty() {
            return None;
        }

        if legal_moves.len() == 1 {
            return Some(legal_moves.as_slice()[0]);
        }

        legal_moves.as_mut_slice().sort_by_key(|m| -mvv_lva_score(m, board));
        if let Some(entry) = state.tt.probe(board.hash()) {
            if let Some(hm) = &entry.best_move {
                if let Some(pos) = legal_moves.as_slice().iter().position(|m| m == hm) {
                    legal_moves.as_mut_slice().swap(0, pos);
                }
            }
        }

        let mut new_best_move = None;

        for m in legal_moves.iter() {
            if limits.start_time.elapsed() + SAFETY_MARGIN >= limits.max_time {
                break;
            }

            let info = board.make_move(m);
            let score = -board.negamax(state, depth - 1, -beta, -alpha);
            board.unmake_move(m, info);

            if score > best_score {
                best_score = score;
                new_best_move = Some(*m);
            }

            alpha = alpha.max(best_score);
        }

        if limits.start_time.elapsed() + SAFETY_MARGIN < limits.max_time {
            best_move = new_best_move;
            last_depth_time = depth_start.elapsed();
            depth += 1;
        } else {
            break;
        }
    }

    best_move
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
    }

    #[test]
    fn test_halfmove_resets_on_pawn_move() {
        let mut board = Board::from_fen("8/8/8/8/8/8/4P3/K1k5 w - - 99 1");
        let mv = find_move(&mut board, Square(1, 4), Square(3, 4), None);
        board.make_move(&mv);
        assert_eq!(board.halfmove_clock(), 0);
        assert!(!board.is_draw());
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
        let score = board.negamax(&mut state, 1, -1000, 1000);
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
