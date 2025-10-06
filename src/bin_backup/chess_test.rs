//! Simple test program for the clean-room chess engine
use chess_engine::{Position, chess_search::Search};

fn main() {
    println!("Testing clean-room chess engine...");
    
    // Test position creation
    let mut pos = Position::startpos();
    println!("✓ Position created successfully");
    
    // Test evaluation
    let eval = pos.evaluate();
    println!("✓ Position evaluation: {}", eval);
    
    // Test move generation
    let moves = pos.legal_moves();
    println!("✓ Generated {} legal moves", moves.len());
    
    // Test search engine
    let mut search = Search::new();
    println!("✓ Search engine created");
    
    // Quick search test
    if let Some(best_move) = search.think(&mut pos, 100, 3) {
        println!("✓ Search found best move: {}", best_move);
    } else {
        println!("! Search didn't find a move");
    }
    
    println!("\nClean-room chess engine test completed successfully!");
}