use crate::board::Move;

use super::format_uci_move;

pub fn print_ready() {
    println!("readyok");
}

pub fn print_bestmove(best_move: Option<Move>) {
    if let Some(best_move) = best_move {
        let uci_move = format_uci_move(&best_move);
        println!("bestmove {}", uci_move);
    } else {
        println!("bestmove (none)");
    }
}
