//! Static Exchange Evaluation (SEE).
//!
//! Evaluates capture sequences on a single square to determine
//! if a capture is winning, losing, or equal.

use super::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use super::state::Board;
use super::types::{Bitboard, Piece, Square};

/// Piece values for SEE (simpler than eval values)
const SEE_VALUES: [i32; 6] = [
    100,   // Pawn
    320,   // Knight
    330,   // Bishop
    500,   // Rook
    900,   // Queen
    20000, // King
];

impl Board {
    /// Static Exchange Evaluation for a capture move.
    ///
    /// Returns the material balance after all exchanges on the target square.
    /// Positive = winning for the side making the initial capture.
    /// Negative = losing.
    /// Zero = equal exchange.
    ///
    /// # Arguments
    /// * `from` - Source square of the capturing piece
    /// * `to` - Target square (where the capture happens)
    ///
    /// # Returns
    /// Material balance in centipawns from the perspective of the side to move.
    #[must_use]
    pub fn see(&self, from: Square, to: Square) -> i32 {
        // Get the piece being captured
        let captured = match self.piece_at(to) {
            Some((_, piece)) => piece,
            None => {
                // En passant - captured piece is a pawn
                if self.en_passant_target == Some(to) {
                    Piece::Pawn
                } else {
                    return 0; // No capture
                }
            }
        };

        // Get the attacking piece
        let attacker = match self.piece_at(from) {
            Some((_, piece)) => piece,
            None => return 0,
        };

        self.see_impl(from, to, attacker, captured)
    }

    /// SEE implementation with known attacker and victim.
    fn see_impl(&self, from: Square, to: Square, attacker: Piece, victim: Piece) -> i32 {
        // Maximum depth of exchanges (should never be exceeded)
        const MAX_DEPTH: usize = 32;

        // Score at each ply of the exchange
        let mut gain = [0i32; MAX_DEPTH];
        let mut depth = 0;

        // Track which side is moving
        let mut side_to_move = self.white_to_move;

        // Build occupancy that we'll modify as pieces are "removed"
        let mut occupancy = self.all_occupied.0;

        // Get all attackers to the target square
        let mut attackers = self.attackers_to(to, Bitboard(occupancy));

        // Initial capture value
        gain[0] = SEE_VALUES[victim.index()];

        // Current attacker
        let mut current_attacker = attacker;
        let mut from_bb = Bitboard::from_square(from);

        loop {
            // Remove the attacker from the board
            occupancy ^= from_bb.0;
            attackers = Bitboard(attackers.0 & !from_bb.0);

            // Check for x-ray attacks revealed by removing this piece
            // Only sliders can have x-ray attacks
            if current_attacker == Piece::Pawn
                || current_attacker == Piece::Bishop
                || current_attacker == Piece::Queen
            {
                // Diagonal x-ray
                let diag_attackers = self.diagonal_sliders();
                let new_diag = Bitboard(
                    slider_attacks(to.index(), occupancy, true) & diag_attackers.0,
                );
                attackers = Bitboard(attackers.0 | (new_diag.0 & occupancy));
            }

            if current_attacker == Piece::Rook || current_attacker == Piece::Queen {
                // Straight x-ray
                let straight_attackers = self.straight_sliders();
                let new_straight = Bitboard(
                    slider_attacks(to.index(), occupancy, false) & straight_attackers.0,
                );
                attackers = Bitboard(attackers.0 | (new_straight.0 & occupancy));
            }

            // Switch sides
            side_to_move = !side_to_move;

            // Find the least valuable attacker for the side to move
            let side_idx = usize::from(!side_to_move);
            let side_pieces = self.occupied[side_idx];

            // Filter attackers to only include pieces of the side to move
            let side_attackers = Bitboard(attackers.0 & side_pieces.0);

            // If no more attackers, we're done
            if side_attackers.is_empty() {
                break;
            }

            depth += 1;
            if depth >= MAX_DEPTH {
                break;
            }

            // Find least valuable attacker
            let (lva_piece, lva_bb) = self.find_least_valuable_attacker(side_attackers, side_idx);

            // Score from making this capture (negated, as it's opponent's gain)
            gain[depth] = SEE_VALUES[current_attacker.index()] - gain[depth - 1];

            // Pruning: if standing pat is better than any possible continuation, stop
            if (-gain[depth - 1]).max(gain[depth]) < 0 {
                break;
            }

            // Don't capture with king if opponent still has attackers
            if lva_piece == Piece::King {
                let opponent_idx = 1 - side_idx;
                let opponent_attackers = Bitboard(attackers.0 & self.occupied[opponent_idx].0);
                if !opponent_attackers.is_empty() {
                    break;
                }
            }

            current_attacker = lva_piece;
            from_bb = lva_bb;
        }

        // Minimax the gains back up
        while depth > 0 {
            depth -= 1;
            gain[depth] = -(-gain[depth]).max(gain[depth + 1]);
        }

        gain[0]
    }

    /// Get all pieces attacking a square.
    fn attackers_to(&self, sq: Square, occupancy: Bitboard) -> Bitboard {
        let sq_idx = sq.index();
        let mut attackers = Bitboard(0);

        // Pawn attacks (look backwards from target to find attacking pawns)
        let white_pawn_attackers = PAWN_ATTACKS[1][sq_idx] & self.pieces[0][Piece::Pawn.index()].0;
        let black_pawn_attackers = PAWN_ATTACKS[0][sq_idx] & self.pieces[1][Piece::Pawn.index()].0;
        attackers.0 |= white_pawn_attackers | black_pawn_attackers;

        // Knight attacks
        let knight_attackers = KNIGHT_ATTACKS[sq_idx]
            & (self.pieces[0][Piece::Knight.index()].0 | self.pieces[1][Piece::Knight.index()].0);
        attackers.0 |= knight_attackers;

        // King attacks
        let king_attackers = KING_ATTACKS[sq_idx]
            & (self.pieces[0][Piece::King.index()].0 | self.pieces[1][Piece::King.index()].0);
        attackers.0 |= king_attackers;

        // Diagonal attackers (bishops and queens)
        let diag_moves = slider_attacks(sq_idx, occupancy.0, true);
        let diag_pieces = self.diagonal_sliders();
        attackers.0 |= diag_moves & diag_pieces.0;

        // Straight attackers (rooks and queens)
        let straight_moves = slider_attacks(sq_idx, occupancy.0, false);
        let straight_pieces = self.straight_sliders();
        attackers.0 |= straight_moves & straight_pieces.0;

        attackers
    }

    /// Get all diagonal sliding pieces (bishops and queens).
    #[inline]
    fn diagonal_sliders(&self) -> Bitboard {
        Bitboard(
            self.pieces[0][Piece::Bishop.index()].0
                | self.pieces[0][Piece::Queen.index()].0
                | self.pieces[1][Piece::Bishop.index()].0
                | self.pieces[1][Piece::Queen.index()].0,
        )
    }

    /// Get all straight sliding pieces (rooks and queens).
    #[inline]
    fn straight_sliders(&self) -> Bitboard {
        Bitboard(
            self.pieces[0][Piece::Rook.index()].0
                | self.pieces[0][Piece::Queen.index()].0
                | self.pieces[1][Piece::Rook.index()].0
                | self.pieces[1][Piece::Queen.index()].0,
        )
    }

    /// Find the least valuable attacker from a set of attackers.
    /// Returns the piece type and a bitboard with just that piece.
    fn find_least_valuable_attacker(
        &self,
        attackers: Bitboard,
        color_idx: usize,
    ) -> (Piece, Bitboard) {
        // Check each piece type from least to most valuable
        for piece_type in [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ] {
            let piece_attackers =
                Bitboard(attackers.0 & self.pieces[color_idx][piece_type.index()].0);
            if !piece_attackers.is_empty() {
                // Return just the first (LSB) attacker
                let single = Bitboard(piece_attackers.0 & piece_attackers.0.wrapping_neg());
                return (piece_type, single);
            }
        }

        // Should never reach here if attackers is non-empty
        (Piece::Pawn, Bitboard(0))
    }

    /// Quick SEE test: returns true if the capture is likely good (SEE >= threshold).
    ///
    /// This is a faster approximation that can be used for move ordering.
    #[inline]
    #[must_use]
    pub fn see_ge(&self, from: Square, to: Square, threshold: i32) -> bool {
        self.see(from, to) >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_board(fen: &str) -> Board {
        fen.parse().expect("valid fen")
    }

    #[test]
    fn test_see_simple_capture() {
        // White pawn captures black pawn
        let board = make_board("8/8/8/3p4/4P3/8/8/8 w - - 0 1");
        let from = Square::new(3, 4); // e4
        let to = Square::new(4, 3); // d5
        let see = board.see(from, to);
        assert_eq!(see, 100); // Win a pawn
    }

    #[test]
    fn test_see_bad_capture() {
        // White pawn captures defended pawn
        let board = make_board("8/8/2p5/3p4/4P3/8/8/8 w - - 0 1");
        let from = Square::new(3, 4); // e4
        let to = Square::new(4, 3); // d5
        let see = board.see(from, to);
        assert_eq!(see, 0); // Equal exchange: pawn takes pawn, pawn recaptures
    }

    #[test]
    fn test_see_winning_exchange() {
        // Knight takes pawn defended by pawn - bad for knight
        let board = make_board("8/8/2p5/3p4/4N3/8/8/8 w - - 0 1");
        let from = Square::new(3, 4); // e4 knight
        let to = Square::new(4, 3); // d5 pawn
        let see = board.see(from, to);
        assert!(see < 0); // Lose: 100 - 320 = -220
    }

    #[test]
    fn test_see_queen_takes_defended_pawn() {
        // Queen takes pawn defended by pawn
        let board = make_board("8/8/2p5/3p4/4Q3/8/8/8 w - - 0 1");
        let from = Square::new(3, 4); // e4 queen
        let to = Square::new(4, 3); // d5 pawn
        let see = board.see(from, to);
        assert!(see < 0); // Very bad: 100 - 900 = -800
    }

    #[test]
    fn test_see_with_xray() {
        // Rook takes rook, x-ray rook recaptures
        let board = make_board("3r4/8/8/8/8/8/8/R2R4 w - - 0 1");
        let from = Square::new(0, 0); // a1 rook
        let to = Square::new(7, 3); // d8 rook
        let see = board.see(from, to);
        // White Rxd8, if black had another attacker it would recapture
        // But black has no recapture, so SEE = 500 (win rook)
        assert_eq!(see, 500);
    }
}
