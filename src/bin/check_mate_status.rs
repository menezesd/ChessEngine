use std::env;

use chess_engine::board::Board;
use chess_engine::uci::{format_uci_move, parse_position_command};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        eprintln!("usage: check_mate_status <move1> <move2> ...");
        return;
    }

    let mut board = Board::new();
    let mut parts: Vec<&str> = Vec::new();
    parts.push("position");
    parts.push("startpos");
    parts.push("moves");
    for mv in args.iter().skip(1) {
        parts.push(mv.as_str());
    }

    parse_position_command(&mut board, &parts);

    let legal_moves = board.generate_moves();
    let in_checkmate = board.is_checkmate();
    let in_stalemate = board.is_stalemate();
    println!(
        "side_to_move: {}",
        if board.white_to_move() { "white" } else { "black" }
    );
    println!("legal_moves: {}", legal_moves.len());
    println!("checkmate: {}", in_checkmate);
    println!("stalemate: {}", in_stalemate);
    for mv in legal_moves.iter() {
        println!("{}", format_uci_move(mv));
    }
}
