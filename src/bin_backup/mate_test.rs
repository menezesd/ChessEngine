use chess_engine::{Position};
use chess_engine::chess_search::Search;

fn main() {
    // Mate in 1: White to move, Qh7#
    let fen = "6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1";
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new();
    search.hash_mb = 16; search.tt.resize(16);
    search.multipv = 1;
    let best = search.think(&mut pos, 5000, 8);
    println!("Mate-in-1 test: FEN: {}", fen);
    println!("Best move: {:?}", best);
    println!("PV: {:?}", search.last_pv_line);
    // Play out PV
    let mut pv_pos = Position::from_fen(fen).unwrap();
    let mut legal = pv_pos.legal_moves();
    let mut mated = false;
    for mv in &search.last_pv_line {
        if !legal.contains(mv) { break; }
        let undo = pv_pos.make_move(mv);
        legal = pv_pos.legal_moves();
        if legal.is_empty() && pv_pos.is_in_check() { mated = true; break; }
    }
    println!("Mate detected by PV follow: {}", mated);
}
