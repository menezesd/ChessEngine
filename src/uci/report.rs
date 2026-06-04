use crate::board::{Move, SearchResult};

use super::format_uci_move;

const NULL_MOVE_UCI: &str = "0000";

pub fn print_ready() {
    println!("readyok");
}

fn bestmove_line(best_move: Option<Move>) -> String {
    if let Some(best_move) = best_move {
        let uci_move = format_uci_move(&best_move);
        format!("bestmove {uci_move}")
    } else {
        format!("bestmove {NULL_MOVE_UCI}")
    }
}

fn bestmove_with_ponder_line(result: SearchResult) -> String {
    match (result.best_move, result.ponder_move) {
        (Some(best), Some(ponder)) => {
            let best_uci = format_uci_move(&best);
            let ponder_uci = format_uci_move(&ponder);
            format!("bestmove {best_uci} ponder {ponder_uci}")
        }
        (Some(best), None) => bestmove_line(Some(best)),
        (None, _) => bestmove_line(None),
    }
}

/// Print best move without ponder
pub fn print_bestmove(best_move: Option<Move>) {
    println!("{}", bestmove_line(best_move));
}

/// Print best move with optional ponder move
pub fn print_bestmove_with_ponder(result: SearchResult) {
    println!("{}", bestmove_with_ponder_line(result));
}

#[cfg(test)]
mod tests {
    use crate::board::{Move, SearchResult, Square};

    use super::{bestmove_line, bestmove_with_ponder_line};

    fn quiet(from: (usize, usize), to: (usize, usize)) -> Move {
        Move::quiet(Square::new(from.0, from.1), Square::new(to.0, to.1))
    }

    #[test]
    fn bestmove_line_formats_null_move_when_absent() {
        assert_eq!(bestmove_line(None), "bestmove 0000");
    }

    #[test]
    fn bestmove_line_formats_move() {
        assert_eq!(bestmove_line(Some(quiet((1, 4), (3, 4)))), "bestmove e2e4");
    }

    #[test]
    fn bestmove_with_ponder_line_formats_ponder_move() {
        let result = SearchResult {
            best_move: Some(quiet((1, 4), (3, 4))),
            ponder_move: Some(quiet((6, 4), (4, 4))),
        };

        assert_eq!(
            bestmove_with_ponder_line(result),
            "bestmove e2e4 ponder e7e5"
        );
    }

    #[test]
    fn bestmove_with_ponder_line_formats_null_move_when_absent() {
        let result = SearchResult {
            best_move: None,
            ponder_move: Some(quiet((6, 4), (4, 4))),
        };

        assert_eq!(bestmove_with_ponder_line(result), "bestmove 0000");
    }
}
