use crate::board::{Move, SearchResult};

use super::format_uci_move;

pub fn print_ready() {
    println!("readyok");
}

/// Print best move without ponder
pub fn print_bestmove(best_move: Option<Move>) {
    if let Some(best_move) = best_move {
        let uci_move = format_uci_move(&best_move);
        println!("bestmove {uci_move}");
    } else {
        println!("bestmove (none)");
    }
}

/// Print best move with optional ponder move
pub fn print_bestmove_with_ponder(result: SearchResult) {
    match (result.best_move, result.ponder_move) {
        (Some(best), Some(ponder)) => {
            let best_uci = format_uci_move(&best);
            let ponder_uci = format_uci_move(&ponder);
            println!("bestmove {best_uci} ponder {ponder_uci}");
        }
        (Some(best), None) => {
            let best_uci = format_uci_move(&best);
            println!("bestmove {best_uci}");
        }
        (None, _) => {
            println!("bestmove (none)");
        }
    }
}
