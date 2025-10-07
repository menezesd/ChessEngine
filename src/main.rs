mod board;
mod constants;
mod magic;
mod search_control;
mod transposition_table;
mod types;
mod uci;
mod uci_info;
mod zobrist;

fn main() {
    uci::run_uci_loop();
}
