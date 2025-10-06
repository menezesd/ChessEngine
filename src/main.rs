// Module declarations
mod bitboard;
mod board;
mod evaluation;
mod perft;
mod search;
mod types;
mod uci;
mod utils;
mod zobrist;

fn main() {
    uci::run_uci_loop();
}
