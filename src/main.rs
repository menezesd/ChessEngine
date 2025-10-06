mod board;
mod constants;
mod transposition_table;
mod types;
mod uci;
mod zobrist;

fn main() {
    uci::run_uci_loop();
}
