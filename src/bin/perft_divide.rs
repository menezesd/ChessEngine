use chess_engine::core::board::Board;
use chess_engine::core::types::format_square;
use std::time::Instant;

fn main() {
    let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    let mut board = Board::try_from_fen(fen).expect("Invalid FEN in perft_divide");
    let depth = 3;
    println!("Perft divide for Kiwipete depth {}", depth);
    let start = Instant::now();
    let mut total = 0u64;
    let mut root_moves: chess_engine::core::types::MoveList = chess_engine::core::types::MoveList::new();
    board.generate_moves_into(&mut root_moves);
    root_moves.sort_by_key(|m| (m.from.0, m.from.1, m.to.0, m.to.1));
    for m in &root_moves {
        let info = board.make_move(m);
        let cnt = chess_engine::perft::Perft::perft(&mut board, depth - 1);
        board.unmake_move(m, info);
        println!(
            "  {}{}: {}",
            format_square(m.from),
            format_square(m.to),
            cnt
        );
        total += cnt;
    }
    let dur = start.elapsed();
    println!("Total: {} in {:?}", total, dur);
}
