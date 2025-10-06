//! Perft testing binary to verify move generation correctness
use chess_engine::{Position, perft::{perft, perft_divide}};

fn main() {
    println!("Chess Engine Perft Testing");
    println!("=========================");
    
    let mut pos = Position::startpos();
    
    println!("\nStarting position perft results:");
    for depth in 1..=4 {
        let nodes = perft(&mut pos, depth);
        println!("perft({}) = {}", depth, nodes);
    }
    
    println!("\nDivide results for depth 1:");
    perft_divide(&mut pos, 1);
    
    // Expected results from chessprogramming.org:
    println!("\nExpected results:");
    println!("perft(1) = 20");
    println!("perft(2) = 400"); 
    println!("perft(3) = 8902");
    println!("perft(4) = 197281");
    println!("perft(5) = 4865609");
}