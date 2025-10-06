use chess_engine::magic_bitboards::{rook_attacks, bishop_attacks, generate_rook_attacks_slow, generate_bishop_attacks_slow};
use std::time::Instant;

fn main() {
    println!("Magic Bitboard Correctness Test");
    println!("==============================");
    
    let mut correct_count = 0;
    let mut incorrect_count = 0;
    let mut total_tests = 0;
    
    // Test rook attacks
    println!("Testing rook attacks...");
    for sq in 0..64 {
        for occupied in [0u64, 0x8040201008040201u64, 0xFFu64, 0xFF00u64, 0xFFFFu64] {
            let magic_result = rook_attacks(sq, occupied);
            let slow_result = generate_rook_attacks_slow(sq, occupied);
            
            total_tests += 1;
            if magic_result == slow_result {
                correct_count += 1;
            } else {
                incorrect_count += 1;
                if incorrect_count <= 5 {
                    println!("Rook mismatch at sq={}, occ={:016X}: magic={:016X}, slow={:016X}", 
                             sq, occupied, magic_result, slow_result);
                }
            }
        }
    }
    
    // Test bishop attacks
    println!("Testing bishop attacks...");
    for sq in 0..64 {
        for occupied in [0u64, 0x8040201008040201u64, 0xFFu64, 0xFF00u64, 0xFFFFu64] {
            let magic_result = bishop_attacks(sq, occupied);
            let slow_result = generate_bishop_attacks_slow(sq, occupied);
            
            total_tests += 1;
            if magic_result == slow_result {
                correct_count += 1;
            } else {
                incorrect_count += 1;
                if incorrect_count <= 10 {
                    println!("Bishop mismatch at sq={}, occ={:016X}: magic={:016X}, slow={:016X}", 
                             sq, occupied, magic_result, slow_result);
                }
            }
        }
    }
    
    println!("\nResults:");
    println!("Total tests: {}", total_tests);
    println!("Correct: {} ({:.1}%)", correct_count, 100.0 * correct_count as f64 / total_tests as f64);
    println!("Incorrect: {} ({:.1}%)", incorrect_count, 100.0 * incorrect_count as f64 / total_tests as f64);
    
    if incorrect_count == 0 {
        println!("✓ Magic bitboards are working correctly!");
    } else {
        println!("✗ Magic bitboards have errors - need to fix implementation");
    }
    
    // Performance comparison
    println!("\nPerformance comparison:");
    
    // Magic performance
    let start = Instant::now();
    let mut dummy = 0u64;
    for _ in 0..100000 {
        for sq in 0..64 {
            dummy ^= rook_attacks(sq, 0x123456789ABCDEF0u64);
        }
    }
    let magic_time = start.elapsed();
    
    // Slow performance  
    let start = Instant::now();
    for _ in 0..100000 {
        for sq in 0..64 {
            dummy ^= generate_rook_attacks_slow(sq, 0x123456789ABCDEF0u64);
        }
    }
    let slow_time = start.elapsed();
    
    println!("Magic method: {:?} ({} ops)", magic_time, 100000 * 64);
    println!("Slow method: {:?} ({} ops)", slow_time, 100000 * 64);
    println!("Speedup: {:.2}x", slow_time.as_nanos() as f64 / magic_time.as_nanos() as f64);
    println!("Dummy result: {}", dummy); // Prevent optimization
}