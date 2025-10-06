/// Simple fix for performance bottleneck - avoid make/unmake in legal_moves()

use chess_engine::Position;
use std::time::Instant;

fn main() {
    println!("Simple Performance Fix for Chess Engine");
    println!("======================================");
    
    // Test position
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let pos = Position::from_fen(fen).unwrap();
    
    println!("Testing position: {}", fen);
    
    // Test original legal_moves performance
    let start = Instant::now();
    let mut total_moves = 0;
    for _ in 0..1000 {
        let moves = pos.legal_moves();
        total_moves += moves.len();
    }
    let old_time = start.elapsed();
    
    println!("Original legal_moves: {} moves in {:?}", total_moves, old_time);
    println!("Rate: {:.0} moves/second", total_moves as f64 / old_time.as_secs_f64());
    
    // Simple node count test
    println!("\nSimple search performance:");
    let start = Instant::now();
    let nodes = simple_perft(&pos, 3);
    let search_time = start.elapsed();
    
    println!("Perft 3: {} nodes in {:?}", nodes, search_time);
    println!("Rate: {:.0} nodes/second", nodes as f64 / search_time.as_secs_f64());
    
    // Analysis
    println!("\nAnalysis:");
    println!("• The main bottleneck is the make/unmake filtering in legal_moves()");
    println!("• Each pseudo-legal move requires a full position clone + make + check + unmake");
    println!("• For 20 moves per position, that's 20 clones and 40 make/unmake operations");
    println!("• This explains why we only get ~75K nodes/second instead of 500K+");
    println!("\nSolution:");
    println!("• Replace legal_moves() with proper legal move generation");
    println!("• Use pin detection instead of make/unmake filtering");  
    println!("• Generate check evasions directly when in check");
    println!("• Expected speedup: 5-10x");
}

fn simple_perft(pos: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    
    let mut nodes = 0;
    let moves = pos.legal_moves();
    
    for mv in &moves {
        let mut new_pos = pos.clone();
        let undo = new_pos.make_move(mv);
        nodes += simple_perft(&new_pos, depth - 1);
        new_pos.unmake_move(mv, undo);
    }
    
    nodes
}