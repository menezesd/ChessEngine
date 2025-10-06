use crate::types::*;
use crate::board::Board;

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

pub fn parse_uci_move(board: &mut Board, uci_string: &str) -> Option<Move> {
    if uci_string.len() < 4 || uci_string.len() > 5 {
        return None;
    }

    let from_chars: Vec<char> = uci_string.chars().take(2).collect();
    let to_chars: Vec<char> = uci_string.chars().skip(2).take(2).collect();

    if from_chars.len() != 2 || to_chars.len() != 2 {
        return None;
    }

    if !('a'..='h').contains(&from_chars[0])
        || !('1'..='8').contains(&from_chars[1])
        || !('a'..='h').contains(&to_chars[0])
        || !('1'..='8').contains(&to_chars[1])
    {
        return None;
    }

    let from_file = file_to_index(from_chars[0]);
    let from_rank = rank_to_index(from_chars[1]);
    let to_file = file_to_index(to_chars[0]);
    let to_rank = rank_to_index(to_chars[1]);

    let from_sq = Square(from_rank, from_file);
    let to_sq = Square(to_rank, to_file);

    let promotion_piece = if uci_string.len() == 5 {
        match uci_string.chars().nth(4) {
            Some('q') => Some(Piece::Queen),
            Some('r') => Some(Piece::Rook),
            Some('b') => Some(Piece::Bishop),
            Some('n') => Some(Piece::Knight),
            _ => return None,
        }
    } else {
        None
    };

    let legal_moves = board.generate_moves();

    for legal_move in legal_moves {
        if legal_move.from == from_sq && legal_move.to == to_sq {
            if legal_move.promotion == promotion_piece {
                return Some(legal_move.clone());
            } else if promotion_piece.is_none() && legal_move.promotion.is_none() {
                return Some(legal_move.clone());
            }
        }
    }

    None
}

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
