use crate::board::*;
use crate::types::*;

// --- Perft (Performance Test) ---

/// Perft results structure to track move counts
pub struct PerftResults {
    pub nodes: u64,
    pub captures: u64,
    pub en_passants: u64,
    pub castles: u64,
    pub promotions: u64,
    pub checks: u64,
    pub checkmates: u64,
}

impl PerftResults {
    pub fn new() -> Self {
        PerftResults {
            nodes: 0,
            captures: 0,
            en_passants: 0,
            castles: 0,
            promotions: 0,
            checks: 0,
            checkmates: 0,
        }
    }
}

/// Main perft function - counts all possible moves to a given depth
pub fn perft(board: &mut Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }

    let mut nodes = 0;
    let moves = board.generate_moves();

    for m in moves {
        let info = board.make_move(&m);
        nodes += perft(board, depth - 1);
        board.unmake_move(&m, info);
    }

    nodes
}

/// Perft divide function - shows move breakdown for debugging
pub fn perft_divide(board: &mut Board, depth: u32) {
    if depth == 0 {
        return;
    }

    let moves = board.generate_moves();
    let mut total_nodes = 0;

    for m in moves {
        let info = board.make_move(&m);
        let nodes = perft(board, depth - 1);
        total_nodes += nodes;
        println!(
            "{}: {}",
            format_square(m.from)
                + &format_square(m.to)
                + if let Some(promo) = m.promotion {
                    match promo {
                        Piece::Queen => "q",
                        Piece::Rook => "r",
                        Piece::Bishop => "b",
                        Piece::Knight => "n",
                        _ => "",
                    }
                } else {
                    ""
                },
            nodes
        );
        board.unmake_move(&m, info);
    }

    println!("\nTotal nodes: {}", total_nodes);
}

/// Perft with detailed statistics
pub fn perft_detailed(board: &mut Board, depth: u32) -> PerftResults {
    let mut results = PerftResults::new();

    if depth == 0 {
        results.nodes = 1;
        return results;
    }

    let moves = board.generate_moves();

    for m in moves {
        let info = board.make_move(&m);

        let sub_results = perft_detailed(board, depth - 1);
        results.nodes += sub_results.nodes;
        results.captures += sub_results.captures;
        results.en_passants += sub_results.en_passants;
        results.castles += sub_results.castles;
        results.promotions += sub_results.promotions;
        results.checks += sub_results.checks;
        results.checkmates += sub_results.checkmates;

        // Count current move statistics
        if m.captured_piece.is_some() {
            results.captures += 1;
        }
        if m.is_en_passant {
            results.en_passants += 1;
        }
        if m.is_castling {
            results.castles += 1;
        }
        if m.promotion.is_some() {
            results.promotions += 1;
        }

        // Check if this move gives check
        if board.is_in_check(if board.white_to_move {
            Color::Black
        } else {
            Color::White
        }) {
            results.checks += 1;
            // Check if it's checkmate (no legal moves for opponent)
            let opponent_moves = board.generate_moves();
            if opponent_moves.is_empty() {
                results.checkmates += 1;
            }
        }

        board.unmake_move(&m, info);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perft_starting_position() {
        let mut board = Board::new();

        // Test perft at depth 1
        assert_eq!(perft(&mut board, 1), 20);

        // Test perft at depth 2
        assert_eq!(perft(&mut board, 2), 400);

        // Test perft at depth 3
        assert_eq!(perft(&mut board, 3), 8902);
    }

    #[test]
    fn test_perft_kiwipete_position() {
        let mut board =
            Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");

        // Test perft at depth 1
        assert_eq!(perft(&mut board, 1), 48);

        // Test perft at depth 2
        assert_eq!(perft(&mut board, 2), 2039);

        // Test perft at depth 3
        assert_eq!(perft(&mut board, 3), 97862);
    }

    #[test]
    fn test_perft_position_3() {
        // Position 3: rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8
        let mut board =
            Board::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8");

        // Test perft at depth 1
        assert_eq!(perft(&mut board, 1), 44);

        // Test perft at depth 2
        assert_eq!(perft(&mut board, 2), 1486);

        // Test perft at depth 3
        assert_eq!(perft(&mut board, 3), 62379);
    }

    #[test]
    fn test_perft_position_4() {
        // Position 4: r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1
        let mut board =
            Board::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1");

        // Test perft at depth 1
        assert_eq!(perft(&mut board, 1), 6);

        // Test perft at depth 2
        assert_eq!(perft(&mut board, 2), 264);

        // Test perft at depth 3
        assert_eq!(perft(&mut board, 3), 9467);
    }

    #[test]
    fn test_perft_position_5() {
        // Position 5: rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8 (same as position 3)
        let mut board =
            Board::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8");

        // Test perft at depth 1
        assert_eq!(perft(&mut board, 1), 44);

        // Test perft at depth 2
        assert_eq!(perft(&mut board, 2), 1486);

        // Test perft at depth 3
        assert_eq!(perft(&mut board, 3), 62379);
    }

    #[test]
    fn test_perft_position_6() {
        // Position 6: r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10
        let mut board = Board::from_fen(
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        );

        // Test perft at depth 1
        assert_eq!(perft(&mut board, 1), 46);

        // Test perft at depth 2
        assert_eq!(perft(&mut board, 2), 2079);

        // Test perft at depth 3
        assert_eq!(perft(&mut board, 3), 89890);
    }

    #[test]
    fn test_perft_starting_position_deeper() {
        let mut board = Board::new();

        // Test deeper perft for starting position
        assert_eq!(perft(&mut board, 4), 197281);
        assert_eq!(perft(&mut board, 5), 4865609);
    }

    #[test]
    fn test_perft_endgame_position() {
        // Simple endgame position: 8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1
        let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1");

        // Test perft at depth 1
        assert_eq!(perft(&mut board, 1), 14);

        // Test perft at depth 2
        assert_eq!(perft(&mut board, 2), 191);

        // Test perft at depth 3
        assert_eq!(perft(&mut board, 3), 2812);
    }

    #[test]
    fn test_perft_castling_position() {
        // Position with castling rights: r3k3/8/8/8/8/8/8/R3K2R w KQ - 0 1
        let mut board = Board::from_fen("r3k3/8/8/8/8/8/8/R3K2R w KQ - 0 1");

        // Test perft at depth 1
        assert_eq!(perft(&mut board, 1), 26);

        // Test perft at depth 2
        assert_eq!(perft(&mut board, 2), 331);

        // Test perft at depth 3
        assert_eq!(perft(&mut board, 3), 8337);
    }

    #[test]
    fn test_perft_promotion_position() {
        // Position with pawn promotion: n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1
        let mut board = Board::from_fen("n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1");

        // Test perft at depth 1
        assert_eq!(perft(&mut board, 1), 24);

        // Test perft at depth 2
        assert_eq!(perft(&mut board, 2), 496);

        // Test perft at depth 3
        assert_eq!(perft(&mut board, 3), 9483);
    }
}
