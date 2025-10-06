//! Debug en passant specifically
use chess_engine::Position;

fn main() {
    // Test the specific en passant position from our test
    let mut pos = Position::startpos();
    if pos.setup_from_fen("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3").is_ok() {
        println!("Successfully parsed FEN");
        println!("Current side: {:?}", pos.side);
        println!("EP file: {:?}", pos.ep_file);
        
        // Debug the specific pawn that should be able to capture en passant
        // White pawn on e5 should be able to capture f6 en passant
        let e5_pawn_sq = 36; // e5 = rank 4 (0-indexed) * 8 + file 4 = 36
        let e5_rank = e5_pawn_sq >> 3; // Should be 4
        println!("E5 pawn square: {}, rank: {}", e5_pawn_sq, e5_rank);
        
        // Check if there's a white pawn on e5
        let white_pawns = pos.pieces[0][0];
        println!("White pawn on e5: {}", (white_pawns & (1u64 << e5_pawn_sq)) != 0);
        
        // Check EP rank calculation  
        let ep_rank = if pos.side == chess_engine::Color::White { 5 } else { 2 };
        println!("Calculated EP rank: {}, e5 rank: {}", ep_rank, e5_rank);
        
        if let Some(ep_file) = pos.ep_file {
            let ep_square = chess_engine::Square(ep_rank * 8 + ep_file);
            println!("EP square: {:?} (should be f6 = 45)", ep_square);
            
            // Check pawn attacks
            let pawn_attacks = chess_engine::magic_bitboards::pawn_attacks(pos.side, chess_engine::Square(e5_pawn_sq as u8));
            println!("Pawn attacks from e5: 0x{:016x}", pawn_attacks);
            println!("EP square bit: 0x{:016x}", chess_engine::magic_bitboards::bit(ep_square));
            println!("Can attack EP square: {}", (pawn_attacks & chess_engine::magic_bitboards::bit(ep_square)) != 0);
        }
        
        let moves = pos.legal_moves();
        let ep_moves: Vec<_> = moves.iter().filter(|m| m.is_ep).collect();
        println!("EN PASSANT MOVES: {:?}", ep_moves);
    }
}