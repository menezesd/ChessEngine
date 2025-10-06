use chess_engine::{ChessPosition, optimized_position::OptimizedPosition};
use std::time::Instant;

fn main() {
    println!("Performance Comparison: Old vs Optimized Move Generation");
    println!("=========================================================");
    
    // Test positions
    let positions = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // Starting position
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", // Complex middle game
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", // Endgame position
    ];
    
    for (i, fen) in positions.iter().enumerate() {
        println!("\nPosition {}: {}", i + 1, fen);
        
        // Create positions
        let old_pos = ChessPosition::from_fen(fen).unwrap();
        let opt_pos = convert_to_optimized(&old_pos);
        
        // Benchmark old move generation
        let start = Instant::now();
        let mut old_moves_total = 0;
        for _ in 0..10000 {
            let moves = old_pos.legal_moves();
            old_moves_total += moves.len();
        }
        let old_time = start.elapsed();
        
        // Benchmark optimized move generation  
        let start = Instant::now();
        let mut opt_moves_total = 0;
        for _ in 0..10000 {
            let moves = opt_pos.fast_legal_moves();
            opt_moves_total += moves.len();
        }
        let opt_time = start.elapsed();
        
        // Verify correctness
        let old_moves = old_pos.legal_moves();
        let opt_moves = opt_pos.fast_legal_moves();
        
        println!("Old implementation: {} moves in {:?} ({:.2} moves/ms)", 
                 old_moves.len(), old_time, old_moves_total as f64 / old_time.as_millis() as f64);
        println!("Optimized implementation: {} moves in {:?} ({:.2} moves/ms)", 
                 opt_moves.len(), opt_time, opt_moves_total as f64 / opt_time.as_millis() as f64);
        
        if old_moves.len() == opt_moves.len() {
            println!("✓ Move count matches");
        } else {
            println!("✗ Move count mismatch! Old: {}, Optimized: {}", old_moves.len(), opt_moves.len());
        }
        
        let speedup = old_time.as_nanos() as f64 / opt_time.as_nanos() as f64;
        println!("Speedup: {:.2}x", speedup);
    }
}

fn convert_to_optimized(pos: &ChessPosition) -> OptimizedPosition {
    // Convert ChessPosition to OptimizedPosition
    // This is a placeholder - you'd need to implement the actual conversion
    OptimizedPosition {
        pieces: pos.pieces,
        occ: pos.occ,
        occ_all: pos.occ_all,
        side: pos.side,
        castle_rights: pos.castle_rights,
        ep_file: pos.ep_file,
        halfmove_clock: pos.halfmove_clock,
        hash: pos.hash,
    }
}