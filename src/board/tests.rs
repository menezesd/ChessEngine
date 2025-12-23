#[cfg(test)]
mod perft_tests {
    use super::super::*;
    use std::time::Instant;

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
        }
    }
}

#[cfg(test)]
mod draw_tests {
    use super::super::*;
    use crate::uci::parse_uci_move;
    use std::sync::atomic::AtomicBool;

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
        let original_rep = board.repetition_counts.get(original_hash);

        let mv = find_move(&mut board, Square(1, 4), Square(3, 4), None);
        let info = board.make_move(&mv);
        board.unmake_move(&mv, info);

        assert_eq!(board.hash(), original_hash);
        assert_eq!(board.castling_rights, original_castling);
        assert_eq!(board.en_passant_target, original_ep);
        assert_eq!(board.halfmove_clock(), original_halfmove);
        assert_eq!(
            board.repetition_counts.get(original_hash),
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

    #[test]
    fn test_fen_round_trip_normalized() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let board = Board::from_fen(fen);
        let out = board.to_fen();
        let in_parts: Vec<&str> = fen.split_whitespace().collect();
        let out_parts: Vec<&str> = out.split_whitespace().collect();
        assert_eq!(&in_parts[..5], &out_parts[..5]);
    }
}

#[cfg(test)]
mod engine_tests {
    use super::super::*;
    use crate::uci::format_uci_move;
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
        let mut board = Board::from_fen(
            "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
        );
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
    fn test_null_move_make_unmake_restores_hash_and_ep() {
        let mut board = Board::from_fen(
            "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
        );
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
        let mut board = Board::from_fen(
            "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
        );
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
        let mut initial_list: Vec<String> = initial_moves
            .iter()
            .map(|m| format_uci_move(m))
            .collect();
        initial_list.sort();

        for mv in initial_moves.iter() {
            let info = board.make_move(mv);
            board.unmake_move(mv, info);
        }

        let after_moves = board.generate_moves();
        let mut after_list: Vec<String> = after_moves.iter().map(|m| format_uci_move(m)).collect();
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
            board.repetition_counts.get(initial_hash),
            initial_rep
        );
    }
}
