use crate::core::board::Board;
use crate::core::types::{file_to_index, format_square, Move, Piece, rank_to_index, Square};
use std::time::Duration;

/// UCI command types that can be parsed from input
#[derive(Debug, Clone)]
pub enum UciCommand {
    Uci,
    IsReady,
    UciNewGame,
    Position {
        fen: Option<String>,
        moves: Vec<String>,
    },
    Go {
        depth: Option<u32>,
        movetime: Option<Duration>,
        wtime: Option<Duration>,
        btime: Option<Duration>,
        winc: Option<Duration>,
        binc: Option<Duration>,
        infinite: bool,
        ponder: bool,
        nodes: Option<u64>,
    },
    Stop,
    PonderHit,
    Quit,
    Display,
}

/// Parse a UCI position command into board state
pub fn parse_position_command(board: &mut Board, parts: &[&str]) {
    let mut i = 1;
    if i < parts.len() && parts[i] == "startpos" {
        *board = Board::new();
        i += 1;
    } else if i < parts.len() && parts[i] == "fen" {
        let fen = parts[i + 1..i + 7].join(" ");
        *board = Board::from_fen(&fen);
        i += 7;
    }

    if i < parts.len() && parts[i] == "moves" {
        i += 1;
        while i < parts.len() {
            if let Some(mv) = parse_uci_move(board, parts[i]) {
                board.make_move(&mv);
            } else {
                eprintln!("Invalid move: {}", parts[i]);
            }
            i += 1;
        }
    }
}

/// Parse a UCI command from a line of input
pub fn parse_uci_command(line: &str) -> Option<UciCommand> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        "uci" => Some(UciCommand::Uci),
        "isready" => Some(UciCommand::IsReady),
        "ucinewgame" => Some(UciCommand::UciNewGame),
        "position" => {
            let mut i = 1;
            let fen = if i < parts.len() && parts[i] == "startpos" {
                i += 1;
                None
            } else if i < parts.len() && parts[i] == "fen" {
                let fen_str = parts[i + 1..i + 7].join(" ");
                i += 7;
                Some(fen_str)
            } else {
                return None;
            };

            let mut moves = Vec::new();
            if i < parts.len() && parts[i] == "moves" {
                i += 1;
                while i < parts.len() {
                    moves.push(parts[i].to_string());
                    i += 1;
                }
            }

            Some(UciCommand::Position { fen, moves })
        }
        "go" => {
            let mut depth = None;
            let mut movetime = None;
            let mut wtime = None;
            let mut btime = None;
            let mut winc = None;
            let mut binc = None;
            let mut infinite = false;
            let mut ponder = false;
            let mut nodes = None;

            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "depth" => {
                        if let Some(d) = parts.get(i + 1).and_then(|s| s.parse::<u32>().ok()) {
                            depth = Some(d);
                        }
                        i += 2;
                    }
                    "movetime" => {
                        if let Some(ms) = parts.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                            movetime = Some(Duration::from_millis(ms));
                        }
                        i += 2;
                    }
                    "wtime" => {
                        if let Some(ms) = parts.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                            wtime = Some(Duration::from_millis(ms));
                        }
                        i += 2;
                    }
                    "btime" => {
                        if let Some(ms) = parts.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                            btime = Some(Duration::from_millis(ms));
                        }
                        i += 2;
                    }
                    "winc" => {
                        if let Some(ms) = parts.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                            winc = Some(Duration::from_millis(ms));
                        }
                        i += 2;
                    }
                    "binc" => {
                        if let Some(ms) = parts.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                            binc = Some(Duration::from_millis(ms));
                        }
                        i += 2;
                    }
                    "infinite" => {
                        infinite = true;
                        i += 1;
                    }
                    "ponder" => {
                        ponder = true;
                        i += 1;
                    }
                    "nodes" => {
                        if let Some(n) = parts.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                            nodes = Some(n);
                        }
                        i += 2;
                    }
                    _ => i += 1,
                }
            }

            Some(UciCommand::Go {
                depth,
                movetime,
                wtime,
                btime,
                winc,
                binc,
                infinite,
                ponder,
                nodes,
            })
        }
        "stop" => Some(UciCommand::Stop),
        "ponderhit" => Some(UciCommand::PonderHit),
        "quit" => Some(UciCommand::Quit),
        "display" | "d" => Some(UciCommand::Display),
        _ => None,
    }
}

/// Format a Move as a UCI move string
pub fn format_uci_move(mv: &Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => '?',
        });
    }
    s
}

/// UCI response types
#[derive(Debug)]
pub enum UciResponse {
    IdName(String),
    IdAuthor(String),
    UciOk,
    ReadyOk,
    BestMove(Option<Move>),
    Info(String),
}

/// Parse a UCI move string into a legal Move for the given board position.
///
/// This function needs `&mut Board` because it calls into move generation to
/// find the legal move that corresponds to the UCI string. Returns `None` if
/// the string is invalid or no legal move matches.
pub fn parse_uci_move(board: &mut Board, uci_string: &str) -> Option<Move> {
    if uci_string.len() < 4 || uci_string.len() > 5 {
        return None; // Invalid length
    }

    let from_chars: Vec<char> = uci_string.chars().take(2).collect();
    let to_chars: Vec<char> = uci_string.chars().skip(2).take(2).collect();

    if from_chars.len() != 2 || to_chars.len() != 2 {
        return None; // Should not happen with length check, but be safe
    }

    // Basic validation of chars
    if !('a'..='h').contains(&from_chars[0])
        || !('1'..='8').contains(&from_chars[1])
        || !('a'..='h').contains(&to_chars[0])
        || !('1'..='8').contains(&to_chars[1])
    {
        return None; // Invalid algebraic notation characters
    }

    let from_file = file_to_index(from_chars[0]);
    let from_rank = rank_to_index(from_chars[1]);
    let to_file = file_to_index(to_chars[0]);
    let to_rank = rank_to_index(to_chars[1]);

    let from_sq = Square(from_rank, from_file);
    let to_sq = Square(to_rank, to_file);

    // Handle promotion
    let promotion_piece = if uci_string.len() == 5 {
        match uci_string.chars().nth(4) {
            Some('q') => Some(Piece::Queen),
            Some('r') => Some(Piece::Rook),
            Some('b') => Some(Piece::Bishop),
            Some('n') => Some(Piece::Knight),
            _ => return None, // Invalid promotion character
        }
    } else {
        None
    };

    // Find the matching legal move
    // We need generate_moves, which takes &mut self. This is slightly awkward
    // if we just want to *find* the move without changing state yet.
    // A temporary clone *might* be acceptable here, or we pass the pre-generated list.
    // Let's generate moves here.
    let mut legal_moves: crate::core::types::MoveList = crate::core::types::MoveList::new();
    board.generate_moves_into(&mut legal_moves);

    for legal_move in legal_moves {
        if legal_move.from == from_sq && legal_move.to == to_sq {
            // Check for promotion match
            if legal_move.promotion == promotion_piece {
                // Found the move! Return a clone of it.
                return Some(legal_move);
            }
            // If no promotion specified by user AND move is not a promotion, it's a match
            else if promotion_piece.is_none() && legal_move.promotion.is_none() {
                return Some(legal_move);
            }
        }
    }

    None // No matching legal move found
}

impl UciResponse {
    /// Format the response as a UCI string
    pub fn to_uci_string(&self) -> String {
        match self {
            UciResponse::IdName(name) => format!("id name {}", name),
            UciResponse::IdAuthor(author) => format!("id author {}", author),
            UciResponse::UciOk => "uciok".to_string(),
            UciResponse::ReadyOk => "readyok".to_string(),
            UciResponse::BestMove(Some(mv)) => format!("bestmove {}", format_uci_move(mv)),
            UciResponse::BestMove(None) => "bestmove 0000".to_string(),
            UciResponse::Info(info) => info.clone(),
        }
    }
}