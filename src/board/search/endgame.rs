//! Exact endgame probes for selected three- and four-piece positions.

mod index;

use self::index::{
    clock_entry, major_index, palette_width, pawn_index, square_color, MinorMaterial,
    MinorPosition, RookPawnPosition, RookPosition, BISHOP_KNIGHT_BLOCK_SIZE,
    BISHOP_PAIR_BLOCK_SIZE, CLOCK_DISTANCE_MASK, CLOCK_STATE_SHIFT, KNIGHT_PAIR_DISTANCE_BITS,
    MAJOR_POSITION_COUNT, NO_WIN, PAWN_POSITION_COUNT, ROOK_BISHOP_BLOCK_SIZE,
    ROOK_KNIGHT_BLOCK_SIZE, ROOK_PAWN_BLOCK_SIZE, ROOK_PAWN_POSITION_COUNT, ROOK_POSITION_COUNT,
    ROOK_QUEEN_BLOCK_SIZE, ROOK_ROOK_BLOCK_SIZE, THREE_PIECE_LENGTH,
};
use super::MATE_SCORE;
use crate::board::attack_tables::{bishop_attacks, rook_attacks, KING_ATTACKS, PAWN_ATTACKS};
use crate::board::draw::{FIFTY_MOVE_RULE_HALFMOVES, THREEFOLD_REPETITION_COUNT};
use crate::board::{Board, Color, Move, MoveList, Piece, MAX_PLY};

const TABLE: &[u8; THREE_PIECE_LENGTH] = include_bytes!(concat!(env!("OUT_DIR"), "/endgames.bin"));
const PAWN_CLOCK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/endgame-clock.bin"));
const BISHOP_KNIGHT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/endgame-bishop-knight.bin"));
const BISHOP_PAIR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/endgame-bishop-pair.bin"));
const KNIGHT_PAIR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/endgame-knight-pair.bin"));
const ROOK_KNIGHT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/endgame-rook-knight.bin"));
const ROOK_BISHOP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/endgame-rook-bishop.bin"));
const ROOK_ROOK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/endgame-rook-rook.bin"));
const ROOK_QUEEN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/endgame-rook-queen.bin"));
const ROOK_PAWN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/endgame-rook-pawn.bin"));
const WINNING_SCORE: i32 = 1_000;

pub(super) enum Outcome {
    Draw,
    Win { side: Color, mate_plies: u8 },
    Conversion { side: Color, plies: u8 },
}

impl Outcome {
    pub(super) fn score(&self, side_to_move: Color, ply: usize) -> i32 {
        let (winner, absolute) = match *self {
            Self::Draw => return 0,
            Self::Win { side, mate_plies } => {
                let distance = ply.saturating_add(usize::from(mate_plies));
                let absolute = if distance <= MAX_PLY {
                    MATE_SCORE - distance as i32
                } else {
                    WINNING_SCORE - i32::from(mate_plies)
                };
                (side, absolute)
            }
            Self::Conversion { side, plies } => (side, WINNING_SCORE - i32::from(plies)),
        };
        if winner == side_to_move {
            absolute
        } else {
            -absolute
        }
    }
}

/// Probe the exact result under the fifty-move rule. A pawn move can
/// preserve a win even when the fastest unbounded mate exceeds the clock.
pub(super) fn probe(board: &Board) -> Option<Outcome> {
    if board.castling_rights != 0 {
        return None;
    }
    match board.all_occupied.popcount() {
        3 => probe_three_piece(board),
        4 => probe_minor_pair(board)
            .or_else(|| probe_rook_ending(board))
            .or_else(|| probe_rook_pawn_fortress(board)),
        _ => None,
    }
}

fn probe_rook_ending(board: &Board) -> Option<Outcome> {
    let rooks = board.all_pieces_of_type(Piece::Rook);
    let to_move = board.side_to_move();
    let shared_material = rooks.popcount() == 2;
    let (rook_side, rook) = if shared_material {
        (
            to_move,
            board.pieces_of(to_move, Piece::Rook).iter().next()?,
        )
    } else {
        let rook = rooks.iter().next()?;
        (board.color_on(rook)?, rook)
    };
    let other_side = rook_side.opponent();
    let (material, other, table, block_size) = if shared_material {
        (
            Piece::Rook,
            board.pieces_of(other_side, Piece::Rook).iter().next()?,
            ROOK_ROOK,
            ROOK_ROOK_BLOCK_SIZE,
        )
    } else if let Some(pawn) = board.pieces_of(other_side, Piece::Pawn).iter().next() {
        if !(1..=6).contains(&pawn.rank()) {
            return None;
        }
        (Piece::Pawn, pawn, ROOK_PAWN, ROOK_PAWN_BLOCK_SIZE)
    } else if let Some(queen) = board.pieces_of(other_side, Piece::Queen).iter().next() {
        (Piece::Queen, queen, ROOK_QUEEN, ROOK_QUEEN_BLOCK_SIZE)
    } else if let Some(bishop) = board.pieces_of(other_side, Piece::Bishop).iter().next() {
        (Piece::Bishop, bishop, ROOK_BISHOP, ROOK_BISHOP_BLOCK_SIZE)
    } else {
        (
            Piece::Knight,
            board.pieces_of(other_side, Piece::Knight).iter().next()?,
            ROOK_KNIGHT,
            ROOK_KNIGHT_BLOCK_SIZE,
        )
    };
    if board.piece_count(rook_side, Piece::King) != 1
        || board.piece_count(other_side, Piece::King) != 1
        || board.is_in_check(to_move.opponent())
    {
        return None;
    }
    let kings = [
        board.find_king(other_side).index(),
        board.find_king(rook_side).index(),
    ];
    let turn = usize::from(to_move == rook_side);
    let index = if material == Piece::Pawn {
        let flip = if other_side == Color::White { 56 } else { 0 };
        let index = RookPawnPosition {
            kings: kings.map(|king| king ^ flip),
            pawn: other.index() ^ flip,
            rook: rook.index() ^ flip,
            turn,
        }
        .index();
        debug_assert!(index < ROOK_PAWN_POSITION_COUNT);
        index
    } else {
        let index = RookPosition {
            kings,
            pieces: [other.index(), rook.index()],
            turn,
        }
        .index();
        debug_assert!(index < ROOK_POSITION_COUNT);
        if shared_material {
            index / 2
        } else {
            index
        }
    };
    let distance = packed_distance(table, index, block_size);
    let plies = distance.unsigned_abs().saturating_sub(1);
    if distance == 0 || plies > remaining_plies(board) {
        return Some(Outcome::Draw);
    }
    Some(if distance == -1 {
        Outcome::Win {
            side: to_move.opponent(),
            mate_plies: 0,
        }
    } else {
        let side = if distance > 0 {
            to_move
        } else {
            to_move.opponent()
        };
        Outcome::Conversion { side, plies }
    })
}

fn packed_distance(table: &[u8], index: usize, block_size: usize) -> i8 {
    let offset = index / block_size * 4;
    let start = u32::from_le_bytes(table[offset..][..4].try_into().unwrap()) as usize;
    let count = table[start];
    if count == 1 {
        return table[start + 1] as i8;
    }
    let width = palette_width(count);
    let bit = (index % block_size) * width;
    let offset = start + 1 + usize::from(count) + bit / 8;
    let word = u16::from_le_bytes([table[offset], table[offset + 1]]);
    let selected = usize::from((word >> (bit % 8)) & ((1 << width) - 1));
    table[start + 1 + selected] as i8
}

fn probe_three_piece(board: &Board) -> Option<Outcome> {
    let (piece, square) = [Piece::Pawn, Piece::Queen, Piece::Rook]
        .into_iter()
        .find_map(|piece| {
            board
                .all_pieces_of_type(piece)
                .iter()
                .next()
                .map(|sq| (piece, sq))
        })?;
    let (side, _) = board.piece_at(square)?;
    if board.piece_count(side, Piece::King) != 1
        || board.piece_count(side.opponent(), Piece::King) != 1
    {
        return None;
    }

    let king = board.find_king(side).index();
    let defender = board.find_king(side.opponent()).index();
    let square_idx = square.index();
    let attacking = board.side_to_move() == side;
    if KING_ATTACKS[king] & (1 << defender) != 0 {
        return None;
    }
    if attacking {
        let occupancy = board.all_occupied.0;
        let attacks = match piece {
            Piece::Pawn => PAWN_ATTACKS[side.index()][square_idx],
            Piece::Rook => rook_attacks(square_idx, occupancy),
            Piece::Queen => {
                rook_attacks(square_idx, occupancy) | bishop_attacks(square_idx, occupancy)
            }
            _ => unreachable!(),
        };
        if attacks & (1 << defender) != 0 {
            return None;
        }
    }
    let budget = remaining_plies(board);
    let mate = match piece {
        Piece::Pawn if (1..=6).contains(&square.rank()) => {
            let flip = if side == Color::Black { 56 } else { 0 };
            let index = pawn_index(king ^ flip, defender ^ flip, square_idx ^ flip, attacking);
            pawn_distance(index, budget)
        }
        Piece::Pawn => return None,
        Piece::Queen | Piece::Rook => {
            let start =
                PAWN_POSITION_COUNT * 2 + usize::from(piece == Piece::Rook) * MAJOR_POSITION_COUNT;
            let distance = TABLE[start + major_index(king, defender, square_idx, attacking)];
            if distance <= budget {
                distance
            } else {
                NO_WIN
            }
        }
        _ => unreachable!(),
    };

    Some(outcome(side, mate))
}

fn remaining_plies(board: &Board) -> u8 {
    FIFTY_MOVE_RULE_HALFMOVES.saturating_sub(board.halfmove_clock()) as u8
}

fn pawn_distance(index: usize, budget: u8) -> u8 {
    if TABLE[index * 2 + 1] > budget {
        return NO_WIN;
    }
    let mate = TABLE[index * 2];
    if mate <= budget {
        return mate;
    }
    let entries = PAWN_CLOCK.as_chunks::<4>().0;
    let target = clock_entry(index, budget, CLOCK_DISTANCE_MASK as u8);
    let upper = entries.partition_point(|&bytes| u32::from_le_bytes(bytes) <= target);
    if let Some(offset) = upper.checked_sub(1) {
        let entry = u32::from_le_bytes(entries[offset]);
        if (entry >> CLOCK_STATE_SHIFT) as usize == index {
            return (entry & CLOCK_DISTANCE_MASK) as u8;
        }
    }
    mate
}

fn probe_minor_pair(board: &Board) -> Option<Outcome> {
    let bishops = board.all_pieces_of_type(Piece::Bishop);
    let (side, material, first, second) = match bishops.popcount() {
        0 => {
            let knights = board.all_pieces_of_type(Piece::Knight);
            let first = knights.iter().next()?;
            let (side, _) = board.piece_at(first)?;
            let second = board.pieces_of(side, Piece::Knight).iter().nth(1)?;
            (side, MinorMaterial::KnightPair, first, second)
        }
        1 => {
            let first = bishops.iter().next()?;
            let (side, _) = board.piece_at(first)?;
            let second = board.pieces_of(side, Piece::Knight).iter().next()?;
            (side, MinorMaterial::BishopKnight, first, second)
        }
        2 => {
            let first = bishops.iter().next()?;
            let (side, _) = board.piece_at(first)?;
            let second = board.pieces_of(side, Piece::Bishop).iter().nth(1)?;
            if square_color(first.index()) == square_color(second.index()) {
                return None;
            }
            (side, MinorMaterial::BishopPair, first, second)
        }
        _ => return None,
    };
    if board.piece_count(side, Piece::King) != 1
        || board.piece_count(side.opponent(), Piece::King) != 1
    {
        return None;
    }

    let king = board.find_king(side).index();
    let defender = board.find_king(side.opponent()).index();
    let attacking = board.side_to_move() == side;
    if KING_ATTACKS[king] & (1 << defender) != 0
        || (attacking && board.is_in_check(side.opponent()))
    {
        return None;
    }
    let state = MinorPosition {
        defender,
        king,
        first: first.index(),
        second: second.index(),
        attacking,
    };
    let mate = minor_pair_distance(state, material);
    Some(outcome(
        side,
        if mate <= remaining_plies(board) {
            mate
        } else {
            NO_WIN
        },
    ))
}

fn minor_pair_distance(state: MinorPosition, material: MinorMaterial) -> u8 {
    let index = state.index(material);
    let (table, block_size) = match material {
        MinorMaterial::BishopKnight => (BISHOP_KNIGHT, BISHOP_KNIGHT_BLOCK_SIZE),
        MinorMaterial::BishopPair => (BISHOP_PAIR, BISHOP_PAIR_BLOCK_SIZE),
        MinorMaterial::KnightPair => {
            let entries = KNIGHT_PAIR.as_chunks::<4>().0;
            return entries
                .binary_search_by_key(&(index as u32), |&bytes| {
                    u32::from_le_bytes(bytes) >> KNIGHT_PAIR_DISTANCE_BITS
                })
                .map_or(NO_WIN, |offset| {
                    (u32::from_le_bytes(entries[offset]) & ((1 << KNIGHT_PAIR_DISTANCE_BITS) - 1))
                        as u8
                });
        }
    };
    let grouped = index / 2 + usize::from(state.attacking) * (material.position_count() / 2);
    packed_distance(table, grouped, block_size) as u8
}

/// A bare king in a rook pawn's promotion corner can hold against a
/// bishop that cannot attack that corner. Every immediate reply is drawn:
/// after a king step, the defender can return or take an unprotected promotion.
fn probe_rook_pawn_fortress(board: &Board) -> Option<Outcome> {
    let pawn = board.all_pieces_of_type(Piece::Pawn).iter().next()?;
    if !matches!(pawn.file(), 0 | 7) || !(1..=6).contains(&pawn.rank()) {
        return None;
    }
    let (side, _) = board.piece_at(pawn)?;
    let bishop = board.pieces_of(side, Piece::Bishop).iter().next()?;
    let defender = side.opponent();
    if board.piece_count(side, Piece::King) != 1 || board.piece_count(defender, Piece::King) != 1 {
        return None;
    }
    let corner = side.pawn_promotion_rank() * 8 + pawn.file();
    let king = board.find_king(side).index();
    if board.find_king(defender).index() == corner
        && square_color(bishop.index()) != square_color(corner)
        && KING_ATTACKS[king] & (1 << corner) == 0
    {
        Some(Outcome::Draw)
    } else {
        None
    }
}

fn outcome(side: Color, mate_plies: u8) -> Outcome {
    if mate_plies == NO_WIN {
        Outcome::Draw
    } else {
        Outcome::Win { side, mate_plies }
    }
}

/// Score a supported ending and account for a defender's immediate
/// repetition claim from the actual game history.
pub(super) fn score(board: &mut Board, ply: usize, legal_history: bool) -> Option<i32> {
    let score = probe(board)?.score(board.side_to_move(), ply);
    if score < 0
        && legal_history
        && board.halfmove_clock() >= 3
        && board.repetition_counts.has_repeated_position()
        && can_claim_repetition(board)
    {
        return Some(0);
    }
    Some(score)
}

/// Can the side to move claim threefold repetition by declaring a legal move?
pub(super) fn can_claim_repetition(board: &mut Board) -> bool {
    can_claim_repetition_in_line(board, &[])
}

pub(super) fn can_claim_repetition_in_line(board: &mut Board, line: &[u64]) -> bool {
    board.generate_moves().into_iter().any(|mv| {
        let undo = board.make_move(mv);
        let hash = board.hash();
        let repeated = board
            .repetition_counts
            .get(hash)
            .saturating_add(line.iter().filter(|&&prior| prior == hash).count() as u32)
            >= THREEFOLD_REPETITION_COUNT;
        board.unmake_move(mv, undo);
        repeated
    })
}

/// Can a quiet check force a winning capture or immediate mate?
pub(super) fn refuted_by_quiet_check(board: &mut Board) -> bool {
    if board.all_occupied.popcount() != 5 {
        return false;
    }
    board
        .generate_moves()
        .into_iter()
        .filter(|mv| mv.is_quiet())
        .any(|mv| {
            let undo = board.make_move(mv);
            let refuted = if board.is_in_check(board.side_to_move()) {
                let mut evasions = MoveList::new();
                board.generate_evasions_into(&mut evasions);
                evasions.into_iter().all(|evasion| {
                    let undo = board.make_move(evasion);
                    let win = has_winning_capture(board);
                    board.unmake_move(evasion, undo);
                    win
                })
            } else {
                false
            };
            board.unmake_move(mv, undo);
            refuted
        })
}

fn has_winning_capture(board: &mut Board) -> bool {
    let mut moves = MoveList::new();
    board.generate_tactical_moves_into(&mut moves);
    moves.into_iter().filter(|mv| mv.is_capture()).any(|mv| {
        let undo = board.make_move(mv);
        let win = board.is_checkmate() || score(board, 0, true).is_some_and(|score| score < 0);
        board.unmake_move(mv, undo);
        win
    })
}

/// Rank by exact outcome, then by mate distance.
pub(super) fn rank_moves(board: &mut Board, moves: &[Move]) -> Option<Vec<(Move, i32)>> {
    probe(board)?;
    let mut ranked = Vec::with_capacity(moves.len());
    for &mv in moves {
        let undo = board.make_move(mv);
        let score = if board.is_checkmate() {
            MATE_SCORE - 1
        } else if board.is_theoretical_draw() {
            0
        } else {
            score(board, 1, true).map_or(0, |score| -score)
        };
        // A pawn advance can preserve the same material class. Rank that
        // reset ahead of a quiet move whose next reset is still to come.
        let score = if board.halfmove_clock() == 0 && score.abs() <= WINNING_SCORE {
            score.signum() * WINNING_SCORE
        } else {
            score
        };
        board.unmake_move(mv, undo);
        ranked.push((mv, score));
    }
    ranked.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
    Some(ranked)
}

pub(super) fn best_reply(board: &mut Board) -> Option<Move> {
    if probe(board).is_none() || board.is_theoretical_draw() {
        return None;
    }
    let moves = board.generate_moves();
    rank_moves(board, moves.as_slice())?
        .first()
        .map(|&(mv, _)| mv)
}

/// Follow the table's optimal replies to the advertised mate.
pub(super) fn mate_line(board: &mut Board, first: Move, length: usize) -> String {
    let mut line = Vec::with_capacity(length);
    let mut next = Some(first);
    while let Some(mv) = next {
        line.push((mv, board.make_move(mv)));
        if line.len() >= length {
            break;
        }
        next = best_reply(board);
    }
    let pv = line
        .iter()
        .map(|(mv, _)| mv.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    for (mv, undo) in line.into_iter().rev() {
        board.unmake_move(mv, undo);
    }
    pv
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::search::{search, SearchConfig, SearchState};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    #[test]
    fn pawn_win_checks_the_clock_to_the_next_pawn_move() {
        for fen in [
            "8/8/8/7k/8/7K/1P6/8 b - - 80 1",
            "8/6p1/k7/8/K7/8/8/8 w - - 80 1",
        ] {
            let board = Board::from_fen(fen);
            assert!(matches!(probe(&board), Some(Outcome::Win { .. })));
            let next_clock = Board::from_fen(&fen.replace("80", "81"));
            assert!(matches!(probe(&next_clock), Some(Outcome::Draw)));
        }
    }

    #[test]
    fn pawn_endgame_reports_the_longer_mate_when_the_clock_forces_a_pawn_move() {
        let fen = "8/8/8/8/8/1K2k3/P7/8 w - - 0 1";
        let board = Board::from_fen(fen);
        let late = Board::from_fen(&fen.replace("0 1", "99 1"));
        assert_eq!(
            probe(&board).unwrap().score(Color::White, 0),
            MATE_SCORE - 29
        );
        assert_eq!(
            probe(&late).unwrap().score(Color::White, 0),
            MATE_SCORE - 33
        );
    }

    #[test]
    fn rook_endgame_with_castling_rights_uses_regular_search() {
        let board = Board::from_fen("3k4/8/8/8/8/8/8/4K2R w K - 0 1");
        assert!(probe(&board).is_none());
    }

    #[test]
    fn knight_pair_ranks_a_mating_blunder_and_follows_the_full_line() {
        let mut board = Board::from_fen("8/8/8/NK6/8/8/1k6/2N5 b - - 98 1");
        let blunder = board.parse_move("b2a3").unwrap();
        let hash = board.hash();
        assert_eq!(
            rank_moves(&mut board, &[blunder]),
            Some(vec![(blunder, -MATE_SCORE + 2)])
        );
        assert_eq!(mate_line(&mut board, blunder, 2), "b2a3 a5c4");
        assert_eq!(board.hash(), hash);

        let mut board = Board::from_fen("8/8/8/NK6/8/8/1k6/2N5 b - - 99 1");
        assert_eq!(rank_moves(&mut board, &[blunder]), Some(vec![(blunder, 0)]));
    }

    #[test]
    fn rook_against_knight_counts_plies_to_conversion() {
        // Kc2 is the only winning move. Winning the knight takes another
        // twenty-two plies against best defence.
        for (fen, winning_move) in [
            ("8/2n5/8/8/8/8/8/k1KR4 w - - 77 1", "c1c2"),
            ("K1kr4/8/8/8/8/8/2N5/8 b - - 77 1", "c8c7"),
        ] {
            let mut board = Board::from_fen(fen);
            let win = board.parse_move(winning_move).unwrap();
            let moves = board.generate_moves();
            assert!(matches!(
                probe(&board),
                Some(Outcome::Conversion { plies: 23, .. })
            ));
            let ranked = rank_moves(&mut board, moves.as_slice()).unwrap();
            assert!(ranked[0].1 > 0);
            assert_eq!(ranked[0].0, win);
            assert!(ranked[1..].iter().all(|&(_, score)| score <= 0));
            let mut late = Board::from_fen(&fen.replace("77 1", "78 1"));
            assert!(matches!(probe(&late), Some(Outcome::Draw)));
            assert_eq!(rank_moves(&mut late, &[win]), Some(vec![(win, 0)]));
        }
    }

    #[test]
    fn equal_rooks_respect_clock_and_color_symmetry() {
        for (fen, winning_move) in [
            ("8/8/8/8/r7/2K5/2R5/k7 w - - 93 1", "c3b3"),
            ("K7/2r5/2k5/R7/8/8/8/8 b - - 93 1", "c6b6"),
        ] {
            let mut board = Board::from_fen(fen);
            assert!(matches!(
                probe(&board),
                Some(Outcome::Conversion { plies: 7, .. })
            ));
            let best_move = board.parse_move(winning_move).unwrap();
            let moves = board.generate_moves();
            let ranked = rank_moves(&mut board, moves.as_slice()).unwrap();
            assert_eq!(ranked[0].0, best_move);
            assert!(ranked[0].1 > 0);
            assert!(ranked[1..].iter().all(|&(_, score)| score <= 0));
            let board = Board::from_fen(&fen.replace("93 1", "94 1"));
            assert!(matches!(probe(&board), Some(Outcome::Draw)));
        }
    }

    #[test]
    fn queen_against_rook_preserves_long_wins_when_the_clock_allows_them() {
        for fen in [
            "8/8/8/8/q7/5K2/8/k3R3 b - - 39 1",
            "K3r3/8/5k2/Q7/8/8/8/8 w - - 39 1",
        ] {
            let mut board = Board::from_fen(fen);
            assert!(matches!(
                probe(&board),
                Some(Outcome::Conversion { plies: 61, .. })
            ));
            let moves = board.generate_moves();
            let ranked = rank_moves(&mut board, moves.as_slice()).unwrap();
            assert_eq!(ranked.iter().filter(|&&(_, score)| score > 0).count(), 2);

            let board = Board::from_fen(&fen.replace("39 1", "40 1"));
            assert!(matches!(probe(&board), Some(Outcome::Draw)));
        }
    }

    #[test]
    fn rook_against_pawn_selects_the_only_win_before_a_draw_claim() {
        for (fen, winning_move) in [
            ("7K/8/p7/8/k7/7R/8/8 w - - 75 1", "h3h7"),
            ("8/8/7r/K7/8/P7/8/7k b - - 75 1", "h6h2"),
        ] {
            let mut board = Board::from_fen(fen);
            assert!(matches!(
                probe(&board),
                Some(Outcome::Conversion { plies: 25, .. })
            ));
            let best = board.parse_move(winning_move).unwrap();
            let moves = board.generate_moves();
            let ranked = rank_moves(&mut board, moves.as_slice()).unwrap();
            assert_eq!(ranked[0].0, best);
            assert!(ranked[0].1 > 0);
            assert!(ranked[1..].iter().all(|&(_, score)| score <= 0));
            let late = Board::from_fen(&fen.replace("75 1", "76 1"));
            assert!(matches!(probe(&late), Some(Outcome::Draw)));
        }
    }

    #[test]
    fn rook_against_pawn_ranks_all_four_promotions() {
        let mut board = Board::from_fen("2R5/8/8/8/4k3/8/4p1K1/8 b - - 99 1");
        let moves = ["e2e1q", "e2e1r", "e2e1b", "e2e1n"].map(|mv| board.parse_move(mv).unwrap());
        let ranked = rank_moves(&mut board, &moves).unwrap();
        assert_eq!(ranked[0], (moves[3], 0));
        assert!(ranked[1..].iter().all(|&(_, score)| score < 0));

        // The defending side has only pawn moves here. At halfmove 99 it
        // cannot declare a reversible move to reach a draw claim.
        let mut board = Board::from_fen("7k/8/7K/8/8/8/p7/6R1 b - - 99 1");
        assert!(matches!(
            probe(&board),
            Some(Outcome::Conversion { plies: 1, .. })
        ));
        let moves = board.generate_moves();
        let ranked = rank_moves(&mut board, moves.as_slice()).unwrap();
        assert!(ranked.iter().all(|&(_, score)| score < 0));
    }

    #[test]
    fn mate_takes_precedence_at_the_fifty_move_boundary() {
        let mut board = Board::from_fen("7k/5K2/8/8/8/8/8/R7 w - - 99 1");
        let moves = board.generate_moves();
        let ranked = rank_moves(&mut board, moves.as_slice()).unwrap();
        assert_eq!(ranked[0].0, board.parse_move("a1h1").unwrap());
        assert_eq!(ranked[0].1, MATE_SCORE - 1);
        assert!(ranked[1..].iter().all(|&(_, score)| score == 0));

        let terminal = Board::from_fen("7k/5K2/8/8/8/8/8/7R b - - 101 1");
        assert_eq!(
            probe(&terminal).unwrap().score(Color::Black, 0),
            -MATE_SCORE
        );
    }

    #[test]
    fn exact_pawn_defense_honors_root_move_restrictions() {
        let mut board = Board::from_fen("8/8/k7/2p5/4K3/8/8/8 w - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let hold = board.parse_move("e4d3").unwrap();
        let lose = board.parse_move("e4e3").unwrap();
        let config = SearchConfig::depth(1);
        let result = search(&mut board, &mut state, config.clone(), &stop);
        assert_eq!(result.best_move, Some(hold));
        let result = search(
            &mut board,
            &mut state,
            config.with_root_moves(vec![lose]),
            &stop,
        );
        assert_eq!(result.best_move, Some(lose));
    }

    #[test]
    fn tablebase_mate_yields_to_the_defenders_repetition_claim() {
        let mut board = Board::from_fen("7k/8/8/8/8/8/8/1Q4K1 w - - 0 1");
        for text in ["b1b2", "h8h7", "b2b1", "h7h8", "b1b2", "h8h7"] {
            let mv = board.parse_move(text).unwrap();
            board.make_move(mv);
        }
        let repeated_line = board.parse_move("b2b1").unwrap();
        let moves = board.generate_moves();
        let ranked = rank_moves(&mut board, moves.as_slice()).unwrap();
        assert!(ranked[0].1 > 0);
        assert_eq!(
            ranked.iter().find(|&&(mv, _)| mv == repeated_line),
            Some(&(repeated_line, 0))
        );
    }

    #[test]
    fn bishop_and_knight_win_only_if_mate_fits_the_clock() {
        let exact = Board::from_fen("8/8/3k4/5nb1/8/8/8/7K b - - 51 1");
        let too_late = Board::from_fen("8/8/3k4/5nb1/8/8/8/7K b - - 52 1");
        assert!(matches!(
            probe(&exact),
            Some(Outcome::Win {
                side: Color::Black,
                mate_plies: 49,
            })
        ));
        assert!(matches!(probe(&too_late), Some(Outcome::Draw)));

        let defended = Board::from_fen("8/8/3k4/5nb1/8/8/8/7K w - - 0 1");
        assert!(matches!(
            probe(&defended),
            Some(Outcome::Win {
                side: Color::Black,
                mate_plies: 50,
                ..
            })
        ));
    }

    #[test]
    fn opposite_color_bishops_convert_against_a_bare_king() {
        let board = Board::from_fen("8/5k2/8/1b1K4/8/8/1b6/8 b - - 0 1");
        assert_eq!(
            probe(&board).unwrap().score(Color::Black, 0),
            MATE_SCORE - 29
        );
        let same_color = Board::from_fen("8/5k2/8/1b1K4/8/8/2b5/8 b - - 0 1");
        assert!(same_color.is_insufficient_material());
        assert!(probe(&same_color).is_none());
    }

    #[test]
    fn wrong_bishop_rook_pawn_fortress_is_drawn_for_either_side_to_move() {
        for fen in [
            "k7/8/P1K5/8/8/8/8/2B5 w - - 0 1",
            "k7/8/P1K5/8/8/8/8/2B5 b - - 0 1",
            "5b2/8/8/8/8/5k1p/8/7K b - - 0 1",
        ] {
            let board = Board::from_fen(fen);
            assert!(matches!(probe(&board), Some(Outcome::Draw)), "{fen}");
        }
        let right_bishop = Board::from_fen("k7/8/P1K5/8/8/8/8/1B6 w - - 0 1");
        assert!(probe(&right_bishop).is_none());
    }

    #[test]
    fn pawn_table_reports_underpromotion_and_a_complete_mate_line() {
        let mut board = Board::from_fen("8/k1P5/2K5/8/8/8/8/8 w - - 0 1");
        let mut state = SearchState::new(1);
        let reports = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&reports);
        let config = SearchConfig::depth(1)
            .with_ponder(true)
            .with_multi_pv(2)
            .with_info_callback(Arc::new(move |info| {
                received
                    .lock()
                    .unwrap()
                    .push((info.multipv, info.mate_in, info.pv.clone()));
            }));
        let original = board.hash();
        let result = search(&mut board, &mut state, config, &AtomicBool::new(false));
        assert_eq!(board.hash(), original);
        assert_eq!(state.stats.nodes, 0);
        assert_eq!(result.best_move, Some(board.parse_move("c7c8r").unwrap()));

        let reports = reports.lock().unwrap();
        let (rank, mate, pv) = &reports[0];
        assert_eq!((*rank, *mate), (1, Some(2)));
        let mut moves = pv.split_whitespace();
        assert_eq!(moves.next(), Some("c7c8r"));
        assert_eq!(
            moves.next().unwrap(),
            result.ponder_move.unwrap().to_string()
        );
        assert_eq!(pv.split_whitespace().count(), 3);
        for token in pv.split_whitespace() {
            let mv = board.parse_move(token).unwrap();
            board.make_move(mv);
        }
        assert!(board.is_checkmate());
        assert_eq!(reports[1].0, 2);
        assert!(reports[1].1.unwrap() > 2);
    }
}
