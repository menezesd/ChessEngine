//! Test special moves (castling, en passant, promotions)
use chess_engine::{Position, perft::perft};

fn main() {
    println!("Testing Special Moves");
    println!("====================");
    
    // Test castling from a position where it's legal
    println!("\n1. Testing castling position:");
    let mut pos = Position::startpos();
    if pos.setup_from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1").is_ok() {
        let moves = pos.legal_moves();
        let castle_moves: Vec<_> = moves.iter().filter(|m| m.is_castle).collect();
        println!("Found {} castling moves: {:?}", castle_moves.len(), castle_moves);
        let perft_result = perft(&mut pos, 1);
        println!("Perft(1) = {}", perft_result);
    } else {
        println!("Failed to parse castling FEN");
    }
    
    // Test en passant 
    println!("\n2. Testing en passant position:");
    let mut pos = Position::startpos();
    if pos.setup_from_fen("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3").is_ok() {
        let moves = pos.legal_moves();
        let ep_moves: Vec<_> = moves.iter().filter(|m| m.is_ep).collect();
        println!("Found {} en passant moves: {:?}", ep_moves.len(), ep_moves);
        let perft_result = perft(&mut pos, 1);
        println!("Perft(1) = {}", perft_result);
    } else {
        println!("Failed to parse en passant FEN");
    }
    
    // Test promotions
    println!("\n3. Testing promotion position:");
    let mut pos = Position::startpos();
    if pos.setup_from_fen("rnbqkb1r/ppppppPp/5n2/8/8/8/PPPPPP1P/RNBQKBNR w KQkq - 0 1").is_ok() {
        let moves = pos.legal_moves();
        let promo_moves: Vec<_> = moves.iter().filter(|m| m.promo.is_some()).collect();
        println!("Found {} promotion moves: {:?}", promo_moves.len(), promo_moves);
        let perft_result = perft(&mut pos, 1);
        println!("Perft(1) = {}", perft_result);
    } else {
        println!("Failed to parse promotion FEN");
    }
    
    // Test starting position still works
    println!("\n4. Verifying starting position still works:");
    let mut pos = Position::startpos();
    let perft_result = perft(&mut pos, 2);
    println!("Starting position perft(2) = {} (should be 400)", perft_result);
}