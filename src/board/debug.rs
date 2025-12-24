use super::{color_index, format_square, piece_index, Bitboard, Board, Color, Piece, Square};

#[cfg(debug_assertions)]
impl Board {
    pub fn print(&self) {
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let piece_char = match self.piece_at(Square(rank, file)) {
                    Some((Color::White, Piece::Pawn)) => 'P',
                    Some((Color::White, Piece::Knight)) => 'N',
                    Some((Color::White, Piece::Bishop)) => 'B',
                    Some((Color::White, Piece::Rook)) => 'R',
                    Some((Color::White, Piece::Queen)) => 'Q',
                    Some((Color::White, Piece::King)) => 'K',
                    Some((Color::Black, Piece::Pawn)) => 'p',
                    Some((Color::Black, Piece::Knight)) => 'n',
                    Some((Color::Black, Piece::Bishop)) => 'b',
                    Some((Color::Black, Piece::Rook)) => 'r',
                    Some((Color::Black, Piece::Queen)) => 'q',
                    Some((Color::Black, Piece::King)) => 'k',
                    None => ' ',
                };
                print!(" {} |", piece_char);
            }
            println!("\n  +---+---+---+---+---+---+---+---+");
        }
        println!("    a   b   c   d   e   f   g   h");
        println!(
            "Turn: {}",
            if self.white_to_move { "White" } else { "Black" }
        );
        if let Some(ep_target) = self.en_passant_target {
            println!("EP Target: {}", format_square(ep_target));
        }
        println!("Castling mask: {:#06b}", self.castling_rights);
        println!("------------------------------------");
    }

    pub fn debug_bitboards(&self) {
        let colors = [Color::White, Color::Black];
        let pieces = [
            (Piece::Pawn, "P"),
            (Piece::Knight, "N"),
            (Piece::Bishop, "B"),
            (Piece::Rook, "R"),
            (Piece::Queen, "Q"),
            (Piece::King, "K"),
        ];

        println!(
            "Side to move: {}",
            if self.white_to_move { "White" } else { "Black" }
        );
        println!("Castling mask: {:#06b}", self.castling_rights);
        if let Some(ep_target) = self.en_passant_target {
            println!("EP Target: {}", format_square(ep_target));
        }
        println!("All occupied: {:#018x}", self.all_occupied.0);

        for color in colors {
            let label = if color == Color::White {
                "White"
            } else {
                "Black"
            };
            for (piece, name) in pieces {
                let bb = self.pieces[color_index(color)][piece_index(piece)].0;
                println!("{} {}: {:#018x}", label, name, bb);
            }
        }
        println!("------------------------------------");
    }

    pub fn print_bitboard_grid(&self, label: &str, bb: Bitboard) {
        println!("{} {:#018x}", label, bb.0);
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let idx = (rank * 8 + file) as u8;
                let ch = if (bb.0 >> idx) & 1 == 1 { '1' } else { '.' };
                print!(" {} |", ch);
            }
            println!("\n  +---+---+---+---+---+---+---+---+");
        }
        println!("    a   b   c   d   e   f   g   h");
        println!("------------------------------------");
    }
}
