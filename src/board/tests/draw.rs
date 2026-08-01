//! Draw detection tests.

use crate::board::{Board, Color, Move, Piece, Square};
use crate::uci::parse_uci_move;

fn find_move(board: &mut Board, from: Square, to: Square, promotion: Option<Piece>) -> Move {
    for m in &board.generate_moves() {
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
fn checkmate_on_fifty_move_boundary_is_not_a_draw() {
    let mut board = Board::from_fen("6k1/5ppp/8/8/8/8/8/4Q2K w - - 99 1");

    apply_uci(&mut board, "e1e8");

    assert_eq!(board.halfmove_clock(), 100);
    assert!(board.is_checkmate());
    assert!(!board.is_draw());
    assert!(!board.is_theoretical_draw());
}

#[test]
fn test_halfmove_resets_on_pawn_move() {
    let mut board = Board::from_fen("8/8/8/8/8/8/4P3/K1k5 w - - 99 1");
    let mv = find_move(&mut board, Square::new(1, 4), Square::new(3, 4), None);
    board.make_move(mv);
    assert_eq!(board.halfmove_clock(), 0);
    assert!(!board.is_draw());
    assert!(!board.is_theoretical_draw());
}

#[test]
fn fullmove_number_advances_after_black_move_and_unmakes() {
    let mut board = Board::new();
    assert_eq!(board.fullmove_number(), 1);

    let white_move = find_move(&mut board, Square::new(1, 4), Square::new(3, 4), None);
    board.make_move(white_move);
    assert_eq!(board.fullmove_number(), 1);

    let black_move = find_move(&mut board, Square::new(6, 4), Square::new(4, 4), None);
    let info = board.make_move(black_move);
    assert_eq!(board.fullmove_number(), 2);

    board.unmake_move(black_move, info);
    assert_eq!(board.fullmove_number(), 1);
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
fn non_capturable_en_passant_does_not_change_position_hash() {
    let with_ep = Board::from_fen("7k/8/8/8/4P3/8/8/K7 b - e3 0 1");
    let without_ep = Board::from_fen("7k/8/8/8/4P3/8/8/K7 b - - 0 1");

    assert_eq!(with_ep.hash(), without_ep.hash());
}

#[test]
fn capturable_en_passant_changes_position_hash() {
    let with_ep = Board::from_fen("7k/8/8/8/3pP3/8/8/K7 b - e3 0 1");
    let without_ep = Board::from_fen("7k/8/8/8/3pP3/8/8/K7 b - - 0 1");

    assert_ne!(with_ep.hash(), without_ep.hash());
}

#[test]
fn pinned_en_passant_does_not_change_position_hash() {
    // e5xd6 e.p. would expose the e-file rook to White's king, so the pawn
    // is adjacent but the capture is illegal and must not affect repetition.
    let mut with_ep = Board::from_fen("k3r3/8/8/3pP3/8/8/8/4K3 w - d6 0 1");
    let without_ep = Board::from_fen("k3r3/8/8/3pP3/8/8/8/4K3 w - - 0 1");

    assert_eq!(with_ep.hash(), without_ep.hash());
    assert!(with_ep
        .generate_moves()
        .iter()
        .all(|mv| !mv.is_en_passant()));
}

#[test]
fn pinned_en_passant_double_push_matches_canonical_position_hash() {
    let mut board = Board::from_fen("k3r3/3p4/8/4P3/8/8/8/4K3 b - - 0 1");
    apply_uci(&mut board, "d7d5");

    let canonical = Board::from_fen("k3r3/8/8/3pP3/8/8/8/4K3 w - - 0 2");
    assert_eq!(board.hash(), canonical.hash());
}

#[test]
fn threefold_counts_a_repeated_position_after_uncapturable_en_passant() {
    let mut board = Board::new();
    apply_uci(&mut board, "e2e4");

    for _ in 0..2 {
        apply_uci(&mut board, "g8f6");
        apply_uci(&mut board, "g1f3");
        apply_uci(&mut board, "f6g8");
        apply_uci(&mut board, "f3g1");
    }

    assert!(board.is_draw());
}

#[test]
fn test_insufficient_material_draw() {
    let board = Board::from_fen("8/8/8/8/8/8/6N1/K1k5 w - - 0 1");
    assert!(!board.is_draw());
    assert!(board.is_theoretical_draw());
}

#[test]
fn same_colored_promoted_bishops_are_insufficient_material() {
    // White's bishops on c1 and e3 are both dark-squared. Even with two
    // bishops, neither side can construct mate without access to the other
    // colour complex.
    let board = Board::from_fen("7k/8/8/8/8/4B3/8/2B1K3 w - - 0 1");

    assert!(board.is_theoretical_draw());
}

#[test]
fn two_knights_against_a_bare_king_can_have_mate_in_one() {
    let mut board = Board::from_fen("8/8/8/8/8/2N5/8/k1K1N3 w - - 0 1");
    assert!(!board.is_theoretical_draw());
    assert_eq!(
        board.two_knights_vs_bare_king_side(),
        Some(crate::board::Color::White)
    );

    apply_uci(&mut board, "e1c2");
    assert!(board.is_checkmate());
}

#[test]
fn test_unmake_restores_state() {
    let mut board = Board::new();
    let original_hash = board.hash();
    let original_castling = board.castling_rights;
    let original_ep = board.en_passant_target;
    let original_halfmove = board.halfmove_clock();
    let original_rep = board.repetition_counts.get(original_hash);

    let mv = find_move(&mut board, Square::new(1, 4), Square::new(3, 4), None);
    let info = board.make_move(mv);
    board.unmake_move(mv, info);

    assert_eq!(board.hash(), original_hash);
    assert_eq!(board.castling_rights, original_castling);
    assert_eq!(board.en_passant_target, original_ep);
    assert_eq!(board.halfmove_clock(), original_halfmove);
    assert_eq!(board.repetition_counts.get(original_hash), original_rep);
}

#[test]
fn clear_starts_a_fresh_repetition_history() {
    let mut board = Board::new();
    let original_hash = board.hash();
    apply_uci(&mut board, "g1f3");
    apply_uci(&mut board, "g8f6");
    apply_uci(&mut board, "f3g1");
    apply_uci(&mut board, "f6g8");
    assert_eq!(board.repetition_counts.get(original_hash), 2);

    board.clear();
    assert_eq!(board.repetition_counts.get(board.hash()), 1);
}

#[test]
fn clear_resets_cached_king_squares() {
    let mut board = Board::from_fen("8/6k1/8/8/8/8/3K4/8 w - - 0 1");
    board.clear();

    assert_eq!(
        board.king_square_index(Color::White),
        Square::new(0, 4).index()
    );
    assert_eq!(
        board.king_square_index(Color::Black),
        Square::new(7, 4).index()
    );
}

#[test]
fn flip_side_to_move_starts_fresh_repetition_history() {
    let mut board = Board::new();
    let original_hash = board.hash();

    board.flip_side_to_move();

    assert_ne!(board.hash(), original_hash);
    assert_eq!(board.repetition_counts.get(board.hash()), 1);
    assert_eq!(board.repetition_counts.get(original_hash), 0);
}

#[test]
fn editor_piece_mutations_start_fresh_repetition_history() {
    let mut board = Board::new();
    let original_hash = board.hash();

    board.remove_piece_at(Square::new(1, 0));
    let after_remove = board.hash();
    assert_ne!(after_remove, original_hash);
    assert_eq!(board.repetition_counts.get(after_remove), 1);
    assert_eq!(board.repetition_counts.get(original_hash), 0);

    board.place_piece(Square::new(2, 0), Color::White, Piece::Pawn);
    assert_ne!(board.hash(), after_remove);
    assert_eq!(board.repetition_counts.get(board.hash()), 1);
    assert_eq!(board.repetition_counts.get(after_remove), 0);
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
    assert_eq!(in_parts, out_parts);
}
