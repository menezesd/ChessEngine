mod board;
mod constants;
mod magic;
mod search_control;
mod transposition_table;
mod types;
mod uci;
mod uci_info;
mod zobrist;
mod see;
mod search;
mod ordering;
mod engine;

fn main() {
    uci::run_uci_loop();
}
