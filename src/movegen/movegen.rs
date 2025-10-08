use crate::core::board::Board;
use crate::core::types::{Move, MoveList, Square, Color, Piece};
use crate::core::bitboard::BitboardUtils;
use crate::core::zobrist::{color_to_zobrist_index, piece_to_zobrist_index};

/// Move generation utilities
pub struct MoveGen;

impl MoveGen {
    /// Helper function to get piece bitboard and opponent information
    fn get_piece_info(board: &Board, color: Color, piece: Piece) -> (u64, u64, u64) {
        let color_idx = color_to_zobrist_index(color);
        let opponent_idx = color_to_zobrist_index(match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        });

        let pieces = board.bitboards[color_idx][piece_to_zobrist_index(piece)];
        let friendly = board.occupancy[color_idx];
        let enemies = board.occupancy[opponent_idx];

        (pieces, friendly, enemies)
    }

    /// Generate moves for sliding pieces (bishops, rooks, queens)
    fn generate_sliding_piece_moves(board: &Board, color: Color, piece: Piece, attack_fn: fn(Square, u64) -> u64, moves: &mut MoveList) {
        let (pieces, friendly, _) = Self::get_piece_info(board, color, piece);

        for from_sq_idx in Self::bits_iter(pieces) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let attacks = attack_fn(from, board.all_occupancy) & !friendly;

            for to_sq_idx in Self::bits_iter(attacks) {
                let to = Square(to_sq_idx / 8, to_sq_idx % 8);
                Self::add_move(board, moves, color, from, to, None, false, false);
            }
        }
    }

    /// Generate tactical moves for sliding pieces (only captures)
    fn generate_sliding_piece_tactical_moves(board: &Board, color: Color, piece: Piece, attack_fn: fn(Square, u64) -> u64, moves: &mut MoveList) {
        let (pieces, _, enemies) = Self::get_piece_info(board, color, piece);

        for from_sq_idx in Self::bits_iter(pieces) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let attacks = attack_fn(from, board.all_occupancy) & enemies;

            for to_sq_idx in Self::bits_iter(attacks) {
                let to = Square(to_sq_idx / 8, to_sq_idx % 8);
                Self::add_move(board, moves, color, from, to, None, false, false);
            }
        }
    }

    /// Generate moves for leaper pieces (knights, kings)
    fn generate_leaper_piece_moves(board: &Board, color: Color, piece: Piece, attack_fn: fn(Square) -> u64, moves: &mut MoveList) {
        let (pieces, friendly, _) = Self::get_piece_info(board, color, piece);

        for from_sq_idx in Self::bits_iter(pieces) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let attacks = attack_fn(from) & !friendly;

            for to_sq_idx in Self::bits_iter(attacks) {
                let to = Square(to_sq_idx / 8, to_sq_idx % 8);
                Self::add_move(board, moves, color, from, to, None, false, false);
            }
        }
    }

    /// Generate tactical moves for leaper pieces (only captures)
    fn generate_leaper_piece_tactical_moves(board: &Board, color: Color, piece: Piece, attack_fn: fn(Square) -> u64, moves: &mut MoveList) {
        let (pieces, _, enemies) = Self::get_piece_info(board, color, piece);

        for from_sq_idx in Self::bits_iter(pieces) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let attacks = attack_fn(from) & enemies;

            for to_sq_idx in Self::bits_iter(attacks) {
                let to = Square(to_sq_idx / 8, to_sq_idx % 8);
                Self::add_move(board, moves, color, from, to, None, false, false);
            }
        }
    }

    /// Generate castling moves for the king
    fn generate_castling_moves(board: &Board, color: Color, moves: &mut MoveList) {
        if board.castling_rights != 0 {
            let king_sq = Self::find_king(board, color).unwrap();
            let opponent_color = match color {
                Color::White => Color::Black,
                Color::Black => Color::White,
            };

            // Kingside castling
            if (board.castling_rights & crate::core::bitboard::castling_bit(color, 'K')) != 0 {
                let kingside_clear = match color {
                    Color::White => (board.all_occupancy & 0x60) == 0, // f1, g1 clear
                    Color::Black => (board.all_occupancy & 0x6000000000000000) == 0, // f8, g8 clear
                };
                let kingside_safe = match color {
                    Color::White => {
                        !Self::is_square_attacked(board, Square(0, 4), opponent_color) &&
                        !Self::is_square_attacked(board, Square(0, 5), opponent_color) &&
                        !Self::is_square_attacked(board, Square(0, 6), opponent_color)
                    },
                    Color::Black => {
                        !Self::is_square_attacked(board, Square(7, 4), opponent_color) &&
                        !Self::is_square_attacked(board, Square(7, 5), opponent_color) &&
                        !Self::is_square_attacked(board, Square(7, 6), opponent_color)
                    },
                };

                if kingside_clear && kingside_safe {
                    let to_sq = match color {
                        Color::White => Square(0, 6),
                        Color::Black => Square(7, 6),
                    };
                    Self::add_move(board, moves, color, king_sq, to_sq, None, true, false);
                }
            }

            // Queenside castling
            if (board.castling_rights & crate::core::bitboard::castling_bit(color, 'Q')) != 0 {
                let queenside_clear = match color {
                    Color::White => (board.all_occupancy & 0xE) == 0, // b1, c1, d1 clear
                    Color::Black => (board.all_occupancy & 0xE00000000000000) == 0, // b8, c8, d8 clear
                };
                let queenside_safe = match color {
                    Color::White => {
                        !Self::is_square_attacked(board, Square(0, 4), opponent_color) &&
                        !Self::is_square_attacked(board, Square(0, 3), opponent_color) &&
                        !Self::is_square_attacked(board, Square(0, 2), opponent_color)
                    },
                    Color::Black => {
                        !Self::is_square_attacked(board, Square(7, 4), opponent_color) &&
                        !Self::is_square_attacked(board, Square(7, 3), opponent_color) &&
                        !Self::is_square_attacked(board, Square(7, 2), opponent_color)
                    },
                };

                if queenside_clear && queenside_safe {
                    let to_sq = match color {
                        Color::White => Square(0, 2),
                        Color::Black => Square(7, 2),
                    };
                    Self::add_move(board, moves, color, king_sq, to_sq, None, true, false);
                }
            }
        }
    }
    /// Generate all pseudo-legal moves for the current position
    pub fn generate_pseudo_moves_into(board: &Board, moves: &mut MoveList) {
        moves.clear();
        let color = board.current_color();
        Self::generate_pawn_moves_for(board, color, moves);
        Self::generate_knight_moves_for(board, color, moves);
        Self::generate_bishop_moves_for(board, color, moves);
        Self::generate_rook_moves_for(board, color, moves);
        Self::generate_queen_moves_for(board, color, moves);
        Self::generate_king_moves_for(board, color, moves);
    }

    /// Generate tactical moves (captures and promotions) for the current position
    pub fn generate_tactical_moves_into(board: &Board, moves: &mut MoveList) {
        moves.clear();
        let color = board.current_color();
        Self::generate_pawn_tactical_moves_for(board, color, moves);
        Self::generate_knight_tactical_moves_for(board, color, moves);
        Self::generate_bishop_tactical_moves_for(board, color, moves);
        Self::generate_rook_tactical_moves_for(board, color, moves);
        Self::generate_queen_tactical_moves_for(board, color, moves);
        Self::generate_king_tactical_moves_for(board, color, moves);
    }

    /// Generate pawn moves for a specific color
    pub fn generate_pawn_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        let color_idx = color_to_zobrist_index(color);
        let opponent_color = match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        let opponent_idx = color_to_zobrist_index(opponent_color);
        let pawns = board.bitboards[color_idx][piece_to_zobrist_index(Piece::Pawn)];
        let occupancy = board.all_occupancy;
        let enemies = board.occupancy[opponent_idx];

        // Direction of movement (white: +1, black: -1)
        let direction = if color == Color::White { 1 } else { -1 };
        let start_rank = if color == Color::White { 1 } else { 6 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        for from_sq_idx in Self::bits_iter(pawns) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let to_rank = (from.0 as i32 + direction) as usize;
            let to_file = from.1;

            // Forward moves
            if to_rank <= 7 {
                let to = Square(to_rank, to_file);
                let to_bit = 1u64 << (to.0 * 8 + to.1);

                if (occupancy & to_bit) == 0 {
                    // Single push
                    if to_rank == promotion_rank {
                        // Promotion
                        Self::add_move(board, moves, color, from, to, Some(Piece::Queen), false, false);
                        Self::add_move(board, moves, color, from, to, Some(Piece::Rook), false, false);
                        Self::add_move(board, moves, color, from, to, Some(Piece::Bishop), false, false);
                        Self::add_move(board, moves, color, from, to, Some(Piece::Knight), false, false);
                    } else {
                        Self::add_move(board, moves, color, from, to, None, false, false);

                        // Double push from starting position
                        if from.0 == start_rank {
                            let double_to = Square((from.0 as i32 + 2 * direction) as usize, to_file);
                            let double_to_bit = 1u64 << (double_to.0 * 8 + double_to.1);
                            if (occupancy & double_to_bit) == 0 {
                                Self::add_move(board, moves, color, from, double_to, None, false, false);
                            }
                        }
                    }
                }

                // Captures
                for file_offset in [-1i32, 1i32] {
                    let capture_file = from.1 as i32 + file_offset;
                    if (0..=7).contains(&capture_file) {
                        let capture_to = Square(to_rank, capture_file as usize);
                        let capture_bit = 1u64 << (capture_to.0 * 8 + capture_to.1);

                        if (enemies & capture_bit) != 0 {
                            if to_rank == promotion_rank {
                                Self::add_move(board, moves, color, from, capture_to, Some(Piece::Queen), false, false);
                                Self::add_move(board, moves, color, from, capture_to, Some(Piece::Rook), false, false);
                                Self::add_move(board, moves, color, from, capture_to, Some(Piece::Bishop), false, false);
                                Self::add_move(board, moves, color, from, capture_to, Some(Piece::Knight), false, false);
                            } else {
                                Self::add_move(board, moves, color, from, capture_to, None, false, false);
                            }
                        }

                        // En passant
                        if let Some(ep_target) = board.en_passant_target {
                            if capture_to == ep_target {
                                Self::add_move(board, moves, color, from, capture_to, None, false, true);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Generate tactical moves for pawns (captures and promotions) for a specific color
    pub fn generate_pawn_tactical_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        let color_idx = color_to_zobrist_index(color);
        let opponent_color = match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        let opponent_idx = color_to_zobrist_index(opponent_color);
        let pawns = board.bitboards[color_idx][piece_to_zobrist_index(Piece::Pawn)];
        let enemies = board.occupancy[opponent_idx];

        // Direction of movement (white: +1, black: -1)
        let direction = if color == Color::White { 1 } else { -1 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };

        for from_sq_idx in Self::bits_iter(pawns) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let to_rank = (from.0 as i32 + direction) as usize;
            let to_file = from.1;

            // Forward moves (only promotions)
            if to_rank <= 7 {
                let to = Square(to_rank, to_file);
                let to_bit = 1u64 << (to.0 * 8 + to.1);

                if (board.all_occupancy & to_bit) == 0 && to_rank == promotion_rank {
                    // Promotion
                    Self::add_move(board, moves, color, from, to, Some(Piece::Queen), false, false);
                    Self::add_move(board, moves, color, from, to, Some(Piece::Rook), false, false);
                    Self::add_move(board, moves, color, from, to, Some(Piece::Bishop), false, false);
                    Self::add_move(board, moves, color, from, to, Some(Piece::Knight), false, false);
                }

                // Captures
                for file_offset in [-1i32, 1i32] {
                    let capture_file = from.1 as i32 + file_offset;
                    if (0..=7).contains(&capture_file) {
                        let capture_to = Square(to_rank, capture_file as usize);
                        let capture_bit = 1u64 << (capture_to.0 * 8 + capture_to.1);

                        if (enemies & capture_bit) != 0 {
                            if to_rank == promotion_rank {
                                Self::add_move(board, moves, color, from, capture_to, Some(Piece::Queen), false, false);
                                Self::add_move(board, moves, color, from, capture_to, Some(Piece::Rook), false, false);
                                Self::add_move(board, moves, color, from, capture_to, Some(Piece::Bishop), false, false);
                                Self::add_move(board, moves, color, from, capture_to, Some(Piece::Knight), false, false);
                            } else {
                                Self::add_move(board, moves, color, from, capture_to, None, false, false);
                            }
                        }

                        // En passant
                        if let Some(ep_target) = board.en_passant_target {
                            if capture_to == ep_target {
                                Self::add_move(board, moves, color, from, capture_to, None, false, true);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Generate knight moves for a specific color
    pub fn generate_knight_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_leaper_piece_moves(board, color, Piece::Knight, Board::knight_attacks, moves);
    }

    /// Generate knight tactical moves for a specific color
    pub fn generate_knight_tactical_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_leaper_piece_tactical_moves(board, color, Piece::Knight, Board::knight_attacks, moves);
    }

    /// Generate bishop moves for a specific color
    pub fn generate_bishop_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_sliding_piece_moves(board, color, Piece::Bishop, Board::bishop_attacks, moves);
    }

    /// Generate bishop tactical moves for a specific color
    pub fn generate_bishop_tactical_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_sliding_piece_tactical_moves(board, color, Piece::Bishop, Board::bishop_attacks, moves);
    }

    /// Generate rook moves for a specific color
    pub fn generate_rook_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_sliding_piece_moves(board, color, Piece::Rook, Board::rook_attacks, moves);
    }

    /// Generate rook tactical moves for a specific color
    pub fn generate_rook_tactical_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_sliding_piece_tactical_moves(board, color, Piece::Rook, Board::rook_attacks, moves);
    }

    /// Generate queen moves for a specific color
    pub fn generate_queen_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_sliding_piece_moves(board, color, Piece::Queen, Board::bishop_attacks, moves);
        Self::generate_sliding_piece_moves(board, color, Piece::Queen, Board::rook_attacks, moves);
    }

    /// Generate queen tactical moves for a specific color
    pub fn generate_queen_tactical_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_sliding_piece_tactical_moves(board, color, Piece::Queen, Board::bishop_attacks, moves);
        Self::generate_sliding_piece_tactical_moves(board, color, Piece::Queen, Board::rook_attacks, moves);
    }

    /// Generate king moves for a specific color
    pub fn generate_king_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_leaper_piece_moves(board, color, Piece::King, Board::king_attacks, moves);
        Self::generate_castling_moves(board, color, moves);
    }

    /// Generate king tactical moves for a specific color
    pub fn generate_king_tactical_moves_for(board: &Board, color: Color, moves: &mut MoveList) {
        Self::generate_leaper_piece_tactical_moves(board, color, Piece::King, Board::king_attacks, moves);
    }

    /// Add a move to the move list
    #[allow(clippy::too_many_arguments)]
    pub fn add_move(
        board: &Board,
        moves: &mut MoveList,
        _color: Color,
        from: Square,
        to: Square,
        promotion: Option<Piece>,
        is_castling: bool,
        is_en_passant: bool,
    ) {
        let captured_piece = if is_en_passant {
            Some(Piece::Pawn)
        } else if !is_castling {
            board.piece_at(to).map(|(_c, p)| p)
        } else {
            None
        };

        moves.push(Move {
            from,
            to,
            promotion,
            captured_piece,
            is_castling,
            is_en_passant,
        });
    }

    /// Add moves for leaper pieces (knights, kings)
    pub fn add_leaper_moves<F>(
        board: &Board,
        color: Color,
        pieces: u64,
        attack_fn: F,
        _is_king: bool,
        moves: &mut MoveList,
    ) where
        F: Fn(Square) -> u64,
    {
        let friendly = board.occupancy[color_to_zobrist_index(color)];

        for from_sq_idx in Self::bits_iter(pieces) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let attacks = attack_fn(from) & !friendly;

            for to_sq_idx in Self::bits_iter(attacks) {
                let to = Square(to_sq_idx / 8, to_sq_idx % 8);
                Self::add_move(board, moves, color, from, to, None, false, false);
            }
        }
    }

    /// Add tactical moves for leaper pieces (only captures)
    pub fn add_leaper_tactical_moves<F>(
        board: &Board,
        color: Color,
        pieces: u64,
        attack_fn: F,
        moves: &mut MoveList,
    ) where
        F: Fn(Square) -> u64,
    {
        let opponent_idx = color_to_zobrist_index(match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        });
        let enemies = board.occupancy[opponent_idx];

        for from_sq_idx in Self::bits_iter(pieces) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let attacks = attack_fn(from) & enemies;

            for to_sq_idx in Self::bits_iter(attacks) {
                let to = Square(to_sq_idx / 8, to_sq_idx % 8);
                Self::add_move(board, moves, color, from, to, None, false, false);
            }
        }
    }

    /// Add moves for sliding pieces (bishops, rooks, queens)
    pub fn add_sliding_moves<F>(
        board: &Board,
        color: Color,
        pieces: u64,
        attack_fn: F,
        moves: &mut MoveList,
    ) where
        F: Fn(Square, u64) -> u64,
    {
        let friendly = board.occupancy[color_to_zobrist_index(color)];

        for from_sq_idx in Self::bits_iter(pieces) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let attacks = attack_fn(from, board.all_occupancy) & !friendly;

            for to_sq_idx in Self::bits_iter(attacks) {
                let to = Square(to_sq_idx / 8, to_sq_idx % 8);
                Self::add_move(board, moves, color, from, to, None, false, false);
            }
        }
    }

    /// Add tactical moves for sliding pieces (only captures)
    pub fn add_sliding_tactical_moves<F>(
        board: &Board,
        color: Color,
        pieces: u64,
        attack_fn: F,
        moves: &mut MoveList,
    ) where
        F: Fn(Square, u64) -> u64,
    {
        let opponent_idx = color_to_zobrist_index(match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        });
        let enemies = board.occupancy[opponent_idx];

        for from_sq_idx in Self::bits_iter(pieces) {
            let from = Square(from_sq_idx / 8, from_sq_idx % 8);
            let attacks = attack_fn(from, board.all_occupancy) & enemies;

            for to_sq_idx in Self::bits_iter(attacks) {
                let to = Square(to_sq_idx / 8, to_sq_idx % 8);
                Self::add_move(board, moves, color, from, to, None, false, false);
            }
        }
    }

    /// Iterator over set bits in a bitboard
    pub fn bits_iter(mut bb: u64) -> impl Iterator<Item = usize> {
        std::iter::from_fn(move || {
            if bb == 0 {
                None
            } else {
                let index = bb.trailing_zeros() as usize;
                bb &= bb - 1;
                Some(index)
            }
        })
    }

    /// Find the king of the specified color
    fn find_king(board: &Board, color: Color) -> Option<Square> {
        let color_idx = color_to_zobrist_index(color);
        let king_bb = board.bitboards[color_idx][piece_to_zobrist_index(Piece::King)];
        if king_bb == 0 {
            None
        } else {
            let index = king_bb.trailing_zeros() as usize;
            Some(BitboardUtils::square_from_index(index))
        }
    }

    /// Check if a square is attacked by the opponent
    fn is_square_attacked(board: &Board, square: Square, attacker_color: Color) -> bool {
        let color_idx = color_to_zobrist_index(attacker_color);
        let square_mask = crate::core::types::bitboard_for_square(square);

        let pawns = board.bitboards[color_idx][piece_to_zobrist_index(Piece::Pawn)];
        if attacker_color == Color::White {
            let attacks = ((pawns & BitboardUtils::NOT_FILE_H) << 9) | ((pawns & BitboardUtils::NOT_FILE_A) << 7);
            if attacks & square_mask != 0 {
                return true;
            }
        } else {
            let attacks = ((pawns & BitboardUtils::NOT_FILE_A) >> 9) | ((pawns & BitboardUtils::NOT_FILE_H) >> 7);
            if attacks & square_mask != 0 {
                return true;
            }
        }

        let knights = board.bitboards[color_idx][piece_to_zobrist_index(Piece::Knight)];
        if Board::knight_attacks(square) & knights != 0 {
            return true;
        }

        let kings = board.bitboards[color_idx][piece_to_zobrist_index(Piece::King)];
        if Board::king_attacks(square) & kings != 0 {
            return true;
        }

        let bishop_like = board.bitboards[color_idx][piece_to_zobrist_index(Piece::Bishop)]
            | board.bitboards[color_idx][piece_to_zobrist_index(Piece::Queen)];
        if Board::bishop_attacks(square, board.all_occupancy) & bishop_like != 0 {
            return true;
        }

        let rook_like = board.bitboards[color_idx][piece_to_zobrist_index(Piece::Rook)]
            | board.bitboards[color_idx][piece_to_zobrist_index(Piece::Queen)];
        if Board::rook_attacks(square, board.all_occupancy) & rook_like != 0 {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::board::Board;
    use crate::core::types::{Color, Piece, Square};

    #[test]
    fn test_generate_pseudo_moves_starting_position() {
        let board = Board::new();
        let mut moves = MoveList::new();
        MoveGen::generate_pseudo_moves_into(&board, &mut moves);

        // Starting position should have 20 pseudo-legal moves
        assert_eq!(moves.len(), 20);

        // Check that all moves are for white
        for mv in &moves {
            assert_eq!(board.piece_at(mv.from).unwrap().0, Color::White);
        }
    }

    #[test]
    fn test_generate_knight_moves_starting_position() {
        let board = Board::new();
        let mut moves = MoveList::new();
        MoveGen::generate_knight_moves_for(&board, Color::White, &mut moves);

        // White has 2 knights, each with 2 moves from starting position
        assert_eq!(moves.len(), 4);

        // All moves should be knight moves
        for mv in &moves {
            assert_eq!(board.piece_at(mv.from).unwrap().1, Piece::Knight);
            assert_eq!(board.piece_at(mv.to), None); // Should move to empty squares
        }
    }

    #[test]
    fn test_generate_pawn_moves_starting_position() {
        let board = Board::new();
        let mut moves = MoveList::new();
        MoveGen::generate_pawn_moves_for(&board, Color::White, &mut moves);

        // White has 8 pawns, each with 2 moves (single + double push)
        assert_eq!(moves.len(), 16);

        // All moves should be pawn moves
        for mv in &moves {
            assert_eq!(board.piece_at(mv.from).unwrap().1, Piece::Pawn);
            assert_eq!(mv.promotion, None); // No promotions in starting position
            assert!(!mv.is_castling);
            assert!(!mv.is_en_passant);
        }
    }

    #[test]
    fn test_generate_king_moves_starting_position() {
        let board = Board::new();
        let mut moves = MoveList::new();
        MoveGen::generate_king_moves_for(&board, Color::White, &mut moves);

        // White king has no moves from starting position (surrounded by pawns)
        assert_eq!(moves.len(), 0);
    }

    #[test]
    fn test_generate_tactical_moves_starting_position() {
        let board = Board::new();
        let mut moves = MoveList::new();
        MoveGen::generate_tactical_moves_into(&board, &mut moves);

        // No tactical moves in starting position
        assert_eq!(moves.len(), 0);
    }

    #[test]
    fn test_bits_iter() {
        let mut bits = 0u64;
        bits |= 1 << 0;  // bit 0
        bits |= 1 << 2;  // bit 2
        bits |= 1 << 63; // bit 63

        let collected: Vec<usize> = MoveGen::bits_iter(bits).collect();
        assert_eq!(collected, vec![0, 2, 63]);
    }

    #[test]
    fn test_get_piece_info() {
        let board = Board::new();
        let (pieces, friendly, enemies) = MoveGen::get_piece_info(&board, Color::White, Piece::Pawn);

        // White pawns in starting position
        assert_eq!(pieces.count_ones(), 8);
        // Friendly pieces include all white pieces
        assert!(friendly.count_ones() >= 16); // At least 8 pawns + 8 other pieces
        // Enemies include all black pieces
        assert_eq!(enemies.count_ones(), 16);
    }

    #[test]
    fn test_pawn_promotion() {
        // Set up a position where white pawn can promote
        let board = Board::try_from_fen("8/P7/8/8/8/8/8/8 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        MoveGen::generate_pawn_moves_for(&board, Color::White, &mut moves);

        // Should have 4 promotion moves (queen, rook, bishop, knight)
        assert_eq!(moves.len(), 4);

        for mv in &moves {
            assert_eq!(mv.from, Square(6, 0)); // a7
            assert_eq!(mv.to, Square(7, 0));   // a8
            assert!(mv.promotion.is_some());
            assert!(!mv.is_castling);
            assert!(!mv.is_en_passant);
        }

        // Check that all promotion pieces are represented
        let promotions: Vec<Piece> = moves.iter().filter_map(|m| m.promotion).collect();
        assert_eq!(promotions.len(), 4);
        assert!(promotions.contains(&Piece::Queen));
        assert!(promotions.contains(&Piece::Rook));
        assert!(promotions.contains(&Piece::Bishop));
        assert!(promotions.contains(&Piece::Knight));
    }
}