//! Make/unmake move tests.

use crate::board::{Board, Color, Move, Piece, Square, UnmakeInfo};
use rand::prelude::*;

fn find_move(board: &mut Board, from: Square, to: Square, promotion: Option<Piece>) -> Move {
    for m in board.generate_moves().iter() {
        if m.from() == from && m.to() == to && m.promotion() == promotion {
            return *m;
        }
    }
    panic!("Expected move not found");
}

#[test]
fn test_en_passant_make_unmake() {
    let mut board =
        Board::from_fen("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3");
    let original_hash = board.hash();
    let original_ep = board.en_passant_target;
    let mv = find_move(&mut board, Square::new(4, 4), Square::new(5, 5), None);
    let info = board.make_move(mv);
    board.unmake_move(mv, info);
    assert_eq!(board.hash(), original_hash);
    assert_eq!(board.en_passant_target, original_ep);
}

#[test]
fn test_promotion_make_unmake() {
    let mut board = Board::from_fen("8/P7/8/8/8/8/8/K1k5 w - - 0 1");
    let original_hash = board.hash();
    let mv = find_move(
        &mut board,
        Square::new(6, 0),
        Square::new(7, 0),
        Some(Piece::Queen),
    );
    let info = board.make_move(mv);
    board.unmake_move(mv, info);
    assert_eq!(board.hash(), original_hash);
    assert_eq!(
        board.piece_at(Square::new(6, 0)),
        Some((Color::White, Piece::Pawn))
    );
}

#[test]
fn test_null_move_make_unmake_restores_hash_and_ep() {
    let mut board =
        Board::from_fen("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3");
    let original_hash = board.hash();
    let original_ep = board.en_passant_target;
    let original_side = board.white_to_move;

    let info = board.make_null_move();
    assert_eq!(board.en_passant_target, None);
    assert_ne!(board.hash(), original_hash);
    assert_ne!(board.white_to_move, original_side);

    board.unmake_null_move(info);
    assert_eq!(board.hash(), original_hash);
    assert_eq!(board.en_passant_target, original_ep);
    assert_eq!(board.white_to_move, original_side);
}

#[test]
fn test_null_move_preserves_castling_rights() {
    let mut board = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let original_castling = board.castling_rights;
    let info = board.make_null_move();
    assert_eq!(board.castling_rights, original_castling);
    board.unmake_null_move(info);
    assert_eq!(board.castling_rights, original_castling);
}

#[test]
fn test_legal_moves_stable_after_make_unmake() {
    let mut board = Board::new();
    let initial_moves = board.generate_moves();
    let mut initial_list: Vec<String> = initial_moves.iter().map(|m| m.to_string()).collect();
    initial_list.sort();

    for mv in initial_moves.iter() {
        let info = board.make_move(*mv);
        board.unmake_move(*mv, info);
    }

    let after_moves = board.generate_moves();
    let mut after_list: Vec<String> = after_moves.iter().map(|m| m.to_string()).collect();
    after_list.sort();

    assert_eq!(initial_list, after_list);
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
        let info = board.make_move(mv);
        history.push((mv, info));

        let recomputed = board.calculate_initial_hash();
        assert_eq!(board.hash(), recomputed);
    }

    while let Some((mv, info)) = history.pop() {
        board.unmake_move(mv, info);
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
    let initial_rep = board.repetition_counts.get(initial_hash);

    let mut rng = StdRng::seed_from_u64(0x5EED);
    let mut history: Vec<(Move, UnmakeInfo)> = Vec::new();

    for _ in 0..200 {
        let moves = board.generate_moves();
        if moves.is_empty() {
            break;
        }
        let idx = rng.gen_range(0..moves.len());
        let mv = moves.as_slice()[idx];
        let info = board.make_move(mv);
        history.push((mv, info));
        let recomputed = board.calculate_initial_hash();
        assert_eq!(board.hash(), recomputed);
    }

    while let Some((mv, info)) = history.pop() {
        board.unmake_move(mv, info);
    }

    assert_eq!(board.hash(), initial_hash);
    assert_eq!(board.halfmove_clock(), initial_halfmove);
    assert_eq!(board.castling_rights, initial_castling);
    assert_eq!(board.en_passant_target, initial_ep);
    assert_eq!(board.repetition_counts.get(initial_hash), initial_rep);
}
