use crate::board::{Color, Piece, Square};

use super::command::XBoardCommand;
use super::output::format_error;
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
    /// Return whether the command was consumed and any protocol response.
    pub(super) fn handle_edit_command(&mut self, cmd: &XBoardCommand) -> (bool, Option<String>) {
        match cmd {
            XBoardCommand::Edit => {
                self.edit_mode = true;
                self.move_history.clear();
                self.edit_white_pieces = true;
            }
            XBoardCommand::EditDone if self.edit_mode => {
                if let Err(error) = self.board.validate_king_counts() {
                    return (true, Some(format_error(".", &error.to_string())));
                }
                self.edit_mode = false;
                self.board.reset_repetition_history();
            }
            XBoardCommand::ClearBoard if self.edit_mode => {
                let was_black_to_move = !self.board.white_to_move();
                self.board.clear();
                if was_black_to_move {
                    self.board.flip_side_to_move();
                }
            }
            XBoardCommand::EditColor if self.edit_mode => {
                // CECP's edit-mode "c" is a bare toggle, not a color select:
                // it flips which side subsequent piece-placement commands
                // add to, starting from White.
                self.edit_white_pieces = !self.edit_white_pieces;
            }
            XBoardCommand::EditPiece(piece_str) if self.edit_mode => {
                self.place_piece(piece_str);
            }
            // The protocol parser is intentionally context-free, so SAN-like
            // edit tokens such as `Ke1` arrive as `UserMove`. In edit mode
            // they unambiguously mean piece placement.
            XBoardCommand::UserMove(piece_str) if self.edit_mode => {
                self.place_piece(piece_str);
            }
            _ => return (false, None),
        }
        (true, None)
    }

    /// Place a piece on the board in edit mode (e.g., "Pa2", "Ke1", "x" to remove).
    fn place_piece(&mut self, piece_str: &str) {
        let Some(command) = parse_edit_piece_command(piece_str) else {
            return;
        };

        let color = if self.edit_white_pieces {
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
    use super::{parse_edit_piece_command, parse_square, EditPieceCommand, XBoardCommand};
    use crate::board::{Piece, Square};
    use crate::xboard::state::XBoardHandler;

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
    fn edit_color_command_toggles_placement_side() {
        let mut handler = XBoardHandler::new();
        handler.handle_edit_command(&XBoardCommand::Edit);
        assert!(handler.edit_white_pieces, "edit mode starts on White");

        handler.handle_edit_command(&XBoardCommand::EditColor);
        assert!(!handler.edit_white_pieces, "c toggles to Black");

        handler.handle_edit_command(&XBoardCommand::EditColor);
        assert!(handler.edit_white_pieces, "c toggles back to White");
    }

    #[test]
    fn edit_done_rejects_invalid_king_counts() {
        let mut handler = XBoardHandler::new();
        handler.handle_edit_command(&XBoardCommand::Edit);
        handler.handle_edit_command(&XBoardCommand::ClearBoard);
        handler.handle_edit_command(&XBoardCommand::UserMove("Ke1".to_string()));

        let (consumed, response) = handler.handle_edit_command(&XBoardCommand::EditDone);
        assert!(consumed);
        assert!(response.is_some());
        assert!(handler.edit_mode);

        handler.handle_edit_command(&XBoardCommand::EditColor);
        handler.handle_edit_command(&XBoardCommand::UserMove("Ke8".to_string()));
        let (_, response) = handler.handle_edit_command(&XBoardCommand::EditDone);
        assert!(response.is_none());
        assert!(!handler.edit_mode);
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
