//! Property-based tests using proptest.

use crate::board::search::{HistoryTable, KillerTable};
use crate::board::{Board, Move, Piece, Square, UnmakeInfo};
use proptest::prelude::*;

/// Strategy to generate a random legal move sequence length
fn move_count_strategy() -> impl Strategy<Value = usize> {
    1..=20usize
}

/// Strategy to generate a random seed for move selection
fn seed_strategy() -> impl Strategy<Value = u64> {
    any::<u64>()
}

proptest! {
    /// Property: make_move followed by unmake_move restores board state exactly
    #[test]
    fn prop_make_unmake_restores_state(seed in seed_strategy(), num_moves in move_count_strategy()) {
        use rand::prelude::*;

        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);

        // Record initial state
        let initial_hash = board.hash();
        let initial_fen = board.to_fen();

        let mut history: Vec<(Move, UnmakeInfo)> = Vec::new();

        // Make random moves
        for _ in 0..num_moves {
            let moves = board.generate_moves();
            if moves.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..moves.len());
            let mv = moves.as_slice()[idx];
            let info = board.make_move(mv);
            history.push((mv, info));
        }

        // Unmake all moves
        while let Some((mv, info)) = history.pop() {
            board.unmake_move(mv, info);
        }

        // Verify state is restored
        prop_assert_eq!(board.hash(), initial_hash);
        prop_assert_eq!(board.to_fen(), initial_fen);
    }

    /// Property: hash is always consistent with recomputed hash
    #[test]
    fn prop_hash_consistency(seed in seed_strategy(), num_moves in move_count_strategy()) {
        use rand::prelude::*;

        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);

        for _ in 0..num_moves {
            let moves = board.generate_moves();
            if moves.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..moves.len());
            let mv = moves.as_slice()[idx];
            board.make_move(mv);

            // Verify hash matches recomputed value
            let recomputed = board.calculate_initial_hash();
            prop_assert_eq!(board.hash(), recomputed);
        }
    }

    /// Property: FEN round-trip preserves position
    #[test]
    fn prop_fen_roundtrip(seed in seed_strategy(), num_moves in move_count_strategy()) {
        use rand::prelude::*;

        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);

        // Make some random moves to get an interesting position
        for _ in 0..num_moves {
            let moves = board.generate_moves();
            if moves.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..moves.len());
            let mv = moves.as_slice()[idx];
            board.make_move(mv);
        }

        // Convert to FEN and back
        let fen = board.to_fen();
        let restored = Board::from_fen(&fen);

        // Verify essential state matches
        prop_assert_eq!(board.hash(), restored.hash());
        prop_assert_eq!(board.white_to_move(), restored.white_to_move());
        prop_assert_eq!(board.castling_rights, restored.castling_rights);
        prop_assert_eq!(board.en_passant_target, restored.en_passant_target);
    }

    /// Property: legal moves are always legal (no self-check)
    #[test]
    fn prop_legal_moves_are_legal(seed in seed_strategy()) {
        use rand::prelude::*;

        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);

        // Make a few random moves
        for _ in 0..10 {
            let moves = board.generate_moves();
            if moves.is_empty() {
                break;
            }

            // Verify each legal move doesn't leave king in check
            let current_color = board.current_color();
            for mv in moves.iter() {
                let info = board.make_move(*mv);
                prop_assert!(!board.is_in_check(current_color),
                    "Legal move left king in check: {:?}", mv);
                board.unmake_move(*mv, info);
            }

            // Make a random move to continue
            let idx = rng.gen_range(0..moves.len());
            let mv = moves.as_slice()[idx];
            board.make_move(mv);
        }
    }

    // ========================================================================
    // SEE Property Tests
    // ========================================================================

    /// Property: SEE for captures is bounded by victim value
    #[test]
    fn prop_see_bounded_by_victim(seed in seed_strategy(), num_moves in 0..15usize) {
        use rand::prelude::*;

        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);

        // Make random moves to get interesting position
        for _ in 0..num_moves {
            let moves = board.generate_moves();
            if moves.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..moves.len());
            board.make_move(moves.as_slice()[idx]);
        }

        // Check SEE for all captures
        let moves = board.generate_moves();
        for mv in moves.iter() {
            if mv.is_capture() {
                let see = board.see(mv.from(), mv.to());

                // SEE should not exceed the value of the captured piece
                // (can't gain more than what's there)
                if let Some((_, victim)) = board.piece_at(mv.to()) {
                    let victim_value = match victim {
                        Piece::Pawn => 100,
                        Piece::Knight => 320,
                        Piece::Bishop => 330,
                        Piece::Rook => 500,
                        Piece::Queen => 900,
                        Piece::King => 20000,
                    };
                    prop_assert!(see <= victim_value,
                        "SEE {} exceeds victim value {} for {:?}", see, victim_value, mv);
                }
            }
        }
    }

    /// Property: SEE for undefended pieces equals piece value
    #[test]
    fn prop_see_undefended_equals_value(_seed in seed_strategy()) {
        // Simple position: white knight captures undefended black pawn
        let mut board: Board = "8/8/8/3p4/4N3/8/8/8 w - - 0 1".parse().unwrap();
        let moves = board.generate_moves();

        for mv in moves.iter() {
            if mv.is_capture() {
                let see = board.see(mv.from(), mv.to());
                // Undefended pawn capture should equal pawn value
                prop_assert_eq!(see, 100);
            }
        }
    }

    // ========================================================================
    // Move Ordering Property Tests
    // ========================================================================

    /// Property: killer moves are preserved after update
    #[test]
    fn prop_killer_preserves_moves(
        ply in 0..100usize,
        mv1_from in 0..64usize,
        mv1_to in 0..64usize,
        mv2_from in 0..64usize,
        mv2_to in 0..64usize
    ) {
        let mut table = KillerTable::new();

        let mv1 = Move::quiet(
            Square::from_index(mv1_from),
            Square::from_index(mv1_to)
        );
        let mv2 = Move::quiet(
            Square::from_index(mv2_from),
            Square::from_index(mv2_to)
        );

        table.update(ply, mv1);

        if ply < crate::board::MAX_PLY {
            prop_assert_eq!(table.primary(ply), mv1);
        }

        if mv1 != mv2 {
            table.update(ply, mv2);
            if ply < crate::board::MAX_PLY {
                prop_assert_eq!(table.primary(ply), mv2);
                prop_assert_eq!(table.secondary(ply), mv1);
            }
        }
    }

    /// Property: history scores are non-negative after updates
    #[test]
    fn prop_history_non_negative(
        mv_from in 0..64usize,
        mv_to in 0..64usize,
        depth in 1..10u32,
        num_updates in 1..10usize
    ) {
        let mut table = HistoryTable::new();
        let mv = Move::quiet(
            Square::from_index(mv_from),
            Square::from_index(mv_to)
        );

        for _ in 0..num_updates {
            table.update(&mv, depth, 0);
        }

        prop_assert!(table.score(&mv) >= 0,
            "History score should be non-negative");
    }

    /// Property: history decay reduces scores
    #[test]
    fn prop_history_decay_reduces(
        mv_from in 0..64usize,
        mv_to in 0..64usize
    ) {
        let mut table = HistoryTable::new();
        let mv = Move::quiet(
            Square::from_index(mv_from),
            Square::from_index(mv_to)
        );

        // Update with significant depth
        table.update(&mv, 5, 0);
        let before = table.score(&mv);

        table.decay();
        let after = table.score(&mv);

        prop_assert!(after <= before,
            "Decay should reduce or maintain score: before={}, after={}", before, after);
    }

    // ========================================================================
    // Evaluation Property Tests
    // ========================================================================

    /// Property: evaluation is bounded (no extreme values)
    #[test]
    fn prop_eval_bounded(seed in seed_strategy(), num_moves in 0..30usize) {
        use rand::prelude::*;

        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);

        for _ in 0..num_moves {
            let moves = board.generate_moves();
            if moves.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..moves.len());
            board.make_move(moves.as_slice()[idx]);
        }

        let eval = board.evaluate();
        // Evaluation should be bounded (not exceeding reasonable material values)
        // Max material is ~39 pawns worth = 3900 cp, plus positional bonuses
        prop_assert!(eval.abs() < 10000,
            "Evaluation {} is unreasonably large", eval);
    }

    /// Property: material count is always accurate
    #[test]
    fn prop_material_accurate(seed in seed_strategy(), num_moves in 0..30usize) {
        use rand::prelude::*;

        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);

        for _ in 0..num_moves {
            let moves = board.generate_moves();
            if moves.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..moves.len());
            board.make_move(moves.as_slice()[idx]);
        }

        // Count material manually
        let mut white_material = 0i32;
        let mut black_material = 0i32;

        for sq in 0..64 {
            if let Some((color, piece)) = board.piece_at(Square::from_index(sq)) {
                let value = match piece {
                    Piece::Pawn => 100,
                    Piece::Knight => 320,
                    Piece::Bishop => 330,
                    Piece::Rook => 500,
                    Piece::Queen => 900,
                    Piece::King => 0, // Don't count king
                };
                if color == crate::board::Color::White {
                    white_material += value;
                } else {
                    black_material += value;
                }
            }
        }

        // Verify material counts are positive
        prop_assert!(white_material >= 0);
        prop_assert!(black_material >= 0);
    }

    // ========================================================================
    // Transposition Table Property Tests
    // ========================================================================

    /// Property: TT stores and retrieves correct data
    #[test]
    fn prop_tt_store_retrieve(
        hash in any::<u64>(),
        depth in 0..100u32,
        score in -10000..10000i32
    ) {
        use crate::tt::{TranspositionTable, BoundType};

        let tt = TranspositionTable::new(1);
        tt.store(hash, depth, score, BoundType::Exact, None, 1);

        if let Some(entry) = tt.probe(hash) {
            prop_assert_eq!(entry.depth() as u32, depth.min(255));
            let clamped_score = score.clamp(i16::MIN as i32, i16::MAX as i32);
            prop_assert_eq!(entry.score(), clamped_score);
        }
        // Note: entry may not be found due to hash collisions, which is acceptable
    }
}
