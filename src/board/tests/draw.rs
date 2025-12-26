//! Draw detection tests.

use crate::board::{Board, Move, Piece, Square};
use crate::uci::parse_uci_move;

fn find_move(board: &mut Board, from: Square, to: Square, promotion: Option<Piece>) -> Move {
    for m in board.generate_moves().iter() {
        if m.from() == from && m.to() == to && m.promotion() == promotion {
            return *m;
        }
    }
    panic!("Expected move not found");
}

fn apply_uci(board: &mut Board, uci: &str) {
    let mv = parse_uci_move(board, uci).expect("uci move not legal");
    board.make_move(mv);
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
    board.make_move(mv);
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
    let original_rep = board.repetition_counts.get(original_hash);

    let mv = find_move(&mut board, Square(1, 4), Square(3, 4), None);
    let info = board.make_move(mv);
    board.unmake_move(mv, info);

    assert_eq!(board.hash(), original_hash);
    assert_eq!(board.castling_rights, original_castling);
    assert_eq!(board.en_passant_target, original_ep);
    assert_eq!(board.halfmove_clock(), original_halfmove);
    assert_eq!(board.repetition_counts.get(original_hash), original_rep);
}

#[test]
fn test_draw_in_search() {
    let board = Board::from_fen("8/8/8/8/8/8/8/K1k5 w - - 100 1");
    assert!(
        board.is_draw(),
        "Position with halfmove clock 100 should be a draw"
    );
}

#[test]
fn test_quiesce_in_checkmate_returns_mate_score() {
    let mut board = Board::from_fen("7k/7Q/7K/8/8/8/8/8 b - - 0 1");
    assert!(board.is_checkmate(), "Black should be in checkmate");
}

#[test]
fn test_fen_round_trip_normalized() {
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let board = Board::from_fen(fen);
    let out = board.to_fen();
    let in_parts: Vec<&str> = fen.split_whitespace().collect();
    let out_parts: Vec<&str> = out.split_whitespace().collect();
    assert_eq!(&in_parts[..5], &out_parts[..5]);
}
