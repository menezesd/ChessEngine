use crate::board::{Color, Piece, Square};

use super::command::XBoardCommand;
use super::state::XBoardHandler;

const EDIT_COMMAND_LEN: usize = 3;
const EDIT_REMOVE_PREFIX: u8 = b'x';

#[derive(Clone, Copy)]
enum EditPieceCommand {
    Place { piece: Piece, file: u8, rank: u8 },
    Remove { file: u8, rank: u8 },
}

fn piece_from_edit_char(piece: u8) -> Option<Piece> {
    match piece {
        b'P' => Some(Piece::Pawn),
        b'N' => Some(Piece::Knight),
        b'B' => Some(Piece::Bishop),
        b'R' => Some(Piece::Rook),
        b'Q' => Some(Piece::Queen),
        b'K' => Some(Piece::King),
        _ => None,
    }
}

fn parse_edit_piece_command(piece_str: &str) -> Option<EditPieceCommand> {
    let bytes = piece_str.as_bytes();
    if bytes.len() != EDIT_COMMAND_LEN {
        return None;
    }

    let piece_or_remove = bytes[0];
    let file = bytes[1];
    let rank = bytes[2];

    if piece_or_remove == EDIT_REMOVE_PREFIX {
        return Some(EditPieceCommand::Remove { file, rank });
    }

    piece_from_edit_char(piece_or_remove).map(|piece| EditPieceCommand::Place { piece, file, rank })
}

impl XBoardHandler {
    pub(super) fn handle_edit_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::Edit => {
                self.edit_mode = true;
                self.board.clear();
                self.move_history.clear();
                self.edit_white_to_move = true;
                None
            }
            XBoardCommand::EditDone => {
                self.edit_mode = false;
                if !self.edit_white_to_move {
                    self.board.flip_side_to_move();
                }
                self.board.reset_repetition_history();
                None
            }
            XBoardCommand::ClearBoard => {
                if self.edit_mode {
                    self.board.clear();
                }
                None
            }
            XBoardCommand::EditColor(color) => {
                if self.edit_mode {
                    self.edit_white_to_move = *color == 'W' || *color == 'w';
                }
                None
            }
            XBoardCommand::EditPiece(piece_str) => {
                if self.edit_mode {
                    self.place_piece(piece_str);
                }
                None
            }
            // The protocol parser is intentionally context-free, so SAN-like
            // edit tokens such as `Ke1` arrive as `UserMove`. In edit mode
            // they unambiguously mean piece placement.
            XBoardCommand::UserMove(piece_str) if self.edit_mode => {
                self.place_piece(piece_str);
                None
            }
            _ => None,
        }
    }

    /// Place a piece on the board in edit mode (e.g., "Pa2", "Ke1", "x" to remove).
    fn place_piece(&mut self, piece_str: &str) {
        let Some(command) = parse_edit_piece_command(piece_str) else {
            return;
        };

        let color = if self.edit_white_to_move {
            Color::White
        } else {
            Color::Black
        };

        match command {
            EditPieceCommand::Place { piece, file, rank } => {
                if let Some(sq) = parse_square(file, rank) {
                    self.board.place_piece(sq, color, piece);
                }
            }
            EditPieceCommand::Remove { file, rank } => {
                if let Some(sq) = parse_square(file, rank) {
                    self.board.remove_piece_at(sq);
                }
            }
        }
    }
}

/// Parse a square from file and rank characters (e.g., 'e', '4' -> e4).
fn parse_square(file: u8, rank: u8) -> Option<Square> {
    let file_idx = match file {
        b'a'..=b'h' => usize::from(file - b'a'),
        _ => return None,
    };
    let rank_idx = match rank {
        b'1'..=b'8' => usize::from(rank - b'1'),
        _ => return None,
    };
    Square::try_new(rank_idx, file_idx)
}

#[cfg(test)]
mod tests {
    use super::{parse_edit_piece_command, parse_square, EditPieceCommand};
    use crate::board::{Piece, Square};

    #[test]
    fn parses_place_piece_command() {
        match parse_edit_piece_command("Ne4") {
            Some(EditPieceCommand::Place { piece, file, rank }) => {
                assert_eq!(piece, Piece::Knight);
                assert_eq!(file, b'e');
                assert_eq!(rank, b'4');
            }
            _ => panic!("expected place command"),
        }
    }

    #[test]
    fn parses_remove_piece_command() {
        match parse_edit_piece_command("xe4") {
            Some(EditPieceCommand::Remove { file, rank }) => {
                assert_eq!(file, b'e');
                assert_eq!(rank, b'4');
            }
            _ => panic!("expected remove command"),
        }
    }

    #[test]
    fn rejects_short_or_unknown_edit_commands() {
        assert!(parse_edit_piece_command("").is_none());
        assert!(parse_edit_piece_command("P").is_none());
        assert!(parse_edit_piece_command("Pa2extra").is_none());
        assert!(parse_edit_piece_command("Ze4").is_none());
    }

    #[test]
    fn parses_valid_square_bytes() {
        assert_eq!(parse_square(b'e', b'4'), Square::try_new(3, 4));
    }

    #[test]
    fn rejects_invalid_square_bytes() {
        assert_eq!(parse_square(b'i', b'4'), None);
        assert_eq!(parse_square(b'e', b'9'), None);
    }
}
