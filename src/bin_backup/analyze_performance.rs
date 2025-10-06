use chess_engine::ChessPosition;
use std::time::Instant;

fn main() {
    println!("Chess Engine Performance Analysis");
    println!("================================");
    
    // Test position
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let pos = ChessPosition::from_fen(fen).unwrap();
    
    println!("Testing position: {}", fen);
    
    // Test move generation performance
    println!("\n1. Move Generation Performance:");
    let start = Instant::now();
    let mut total_moves = 0;
    for _ in 0..1000 {
        let moves = pos.legal_moves();
        total_moves += moves.len();
    }
    let move_time = start.elapsed();
    
    println!("Generated {} moves in {:?}", total_moves, move_time);
    println!("Rate: {:.0} moves/second", total_moves as f64 / move_time.as_secs_f64());
    
    // Test individual components
    println!("\n2. Component Analysis:");
    
    // Test make/unmake performance
    let moves = pos.legal_moves();
    println!("Legal moves in position: {}", moves.len());
    
    let start = Instant::now();
    let mut make_unmake_count = 0;
    for _ in 0..1000 {
        for mv in &moves {
            let mut test_pos = pos.clone();
            let undo = test_pos.make_move(mv);
            test_pos.unmake_move(mv, undo);
            make_unmake_count += 1;
        }
    }
    let make_unmake_time = start.elapsed();
    
    println!("Make/unmake operations: {} in {:?}", make_unmake_count, make_unmake_time);
    println!("Rate: {:.0} make/unmake per second", make_unmake_count as f64 / make_unmake_time.as_secs_f64());
    
    // Test attack detection
    let start = Instant::now();
    let king_sq = pos.pieces[0][5].trailing_zeros() as u8; // White king
    let mut attack_tests = 0;
    for _ in 0..10000 {
        let _attacked = pos.attacks_to(chess_engine::Square(king_sq), chess_engine::Color::Black);
        attack_tests += 1;
    }
    let attack_time = start.elapsed();
    
    println!("Attack detection: {} tests in {:?}", attack_tests, attack_time);
    println!("Rate: {:.0} attack tests per second", attack_tests as f64 / attack_time.as_secs_f64());
    
    // Analyze the bottleneck
    println!("\n3. Performance Analysis:");
    let moves_per_legal_call = total_moves / 1000;
    let _make_unmake_per_legal_call = moves_per_legal_call; // Each legal move requires make/unmake
    
    println!("Average moves per legal_moves() call: {}", moves_per_legal_call);
    println!("Estimated time per make/unmake: {:.2}µs", 
             make_unmake_time.as_micros() as f64 / make_unmake_count as f64);
    
    let estimated_legal_moves_time = (make_unmake_time.as_micros() as f64 / make_unmake_count as f64) * moves_per_legal_call as f64;
    println!("Estimated time for make/unmake in legal_moves(): {:.2}µs", estimated_legal_moves_time);
    
    let actual_legal_moves_time = move_time.as_micros() as f64 / 1000.0;
    println!("Actual time per legal_moves() call: {:.2}µs", actual_legal_moves_time);
    
    let overhead_percentage = ((actual_legal_moves_time - estimated_legal_moves_time) / actual_legal_moves_time) * 100.0;
    println!("Non-make/unmake overhead: {:.1}%", overhead_percentage.max(0.0));
    
    // Performance recommendations
    println!("\n4. Optimization Recommendations:");
    println!("• The main bottleneck is the make/unmake filter in legal_moves()");
    println!("• Each legal move requires: clone + make + check + unmake");
    println!("• Implement proper legal move generation without make/unmake");
    println!("• Use pin detection and check evasion generation instead");
    println!("• Expected speedup: 5-10x for move generation");
}