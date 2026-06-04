use crate::board::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece, Square};

impl Board {
    /// Get all pieces attacking a square.
    pub(super) fn attackers_to(&self, sq: Square, occupancy: Bitboard) -> Bitboard {
        let sq_idx = sq.index();
        let mut attackers = Bitboard(0);

        let white_pawn_attackers =
            PAWN_ATTACKS[1][sq_idx] & self.pieces_of(Color::White, Piece::Pawn).0;
        let black_pawn_attackers =
            PAWN_ATTACKS[0][sq_idx] & self.pieces_of(Color::Black, Piece::Pawn).0;
        attackers.0 |= white_pawn_attackers | black_pawn_attackers;

        let knight_attackers = KNIGHT_ATTACKS[sq_idx]
            & (self.pieces_of(Color::White, Piece::Knight).0
                | self.pieces_of(Color::Black, Piece::Knight).0);
        attackers.0 |= knight_attackers;

        let king_attackers = KING_ATTACKS[sq_idx]
            & (self.pieces_of(Color::White, Piece::King).0
                | self.pieces_of(Color::Black, Piece::King).0);
        attackers.0 |= king_attackers;

        let diag_moves = slider_attacks(sq_idx, occupancy.0, true);
        let diag_pieces = self.diagonal_sliders();
        attackers.0 |= diag_moves & diag_pieces.0;

        let straight_moves = slider_attacks(sq_idx, occupancy.0, false);
        let straight_pieces = self.straight_sliders();
        attackers.0 |= straight_moves & straight_pieces.0;

        attackers
    }

    /// Get all diagonal sliding pieces (bishops and queens).
    #[inline]
    pub(super) fn diagonal_sliders(&self) -> Bitboard {
        Bitboard(self.all_pieces_of_type(Piece::Bishop).0 | self.all_pieces_of_type(Piece::Queen).0)
    }

    /// Get all straight sliding pieces (rooks and queens).
    #[inline]
    pub(super) fn straight_sliders(&self) -> Bitboard {
        Bitboard(self.all_pieces_of_type(Piece::Rook).0 | self.all_pieces_of_type(Piece::Queen).0)
    }

    /// Find the least valuable attacker from a set of attackers.
    /// Returns the piece type and a bitboard with just that piece.
    pub(super) fn find_least_valuable_attacker(
        &self,
        attackers: Bitboard,
        color: Color,
    ) -> (Piece, Bitboard) {
        for piece_type in [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ] {
            let piece_attackers = Bitboard(attackers.0 & self.pieces_of(color, piece_type).0);
            if !piece_attackers.is_empty() {
                let single = Bitboard(piece_attackers.0 & piece_attackers.0.wrapping_neg());
                return (piece_type, single);
            }
        }

        (Piece::Pawn, Bitboard(0))
    }
}
