//! Property-based tests using proptest.

use crate::board::{Board, Move, UnmakeInfo};
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
}
