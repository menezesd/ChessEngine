mod board;
mod constants;
mod transposition_table;
mod types;
mod uci;
mod zobrist;
mod magic;
mod uci_info;
mod search_control;

fn main() {
    uci::run_uci_loop();
}
