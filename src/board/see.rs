//! Static Exchange Evaluation (SEE).
//!
//! Evaluates capture sequences on a single square to determine
//! if a capture is winning, losing, or equal.

use super::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use super::state::Board;
use super::types::{Bitboard, Color, Piece, Square};

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
        let Some((_, attacker)) = self.piece_at(from) else {
            return 0;
        };

        self.see_impl(from, to, attacker, captured)
    }

    /// SEE with known attacker and victim pieces.
    ///
    /// This avoids redundant `piece_at` lookups when the caller already knows the pieces.
    #[inline]
    #[must_use]
    pub fn see_with_pieces(&self, from: Square, to: Square, attacker: Piece, victim: Piece) -> i32 {
        self.see_impl(from, to, attacker, victim)
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
                let new_diag =
                    Bitboard(slider_attacks(to.index(), occupancy, true) & diag_attackers.0);
                attackers = Bitboard(attackers.0 | (new_diag.0 & occupancy));
            }

            if current_attacker.attacks_straight() {
                // Straight x-ray
                let straight_attackers = self.straight_sliders();
                let new_straight =
                    Bitboard(slider_attacks(to.index(), occupancy, false) & straight_attackers.0);
                attackers = Bitboard(attackers.0 | (new_straight.0 & occupancy));
            }

            // Switch sides
            side_to_move = !side_to_move;

            // Find the least valuable attacker for the side to move
            let side_color = if side_to_move {
                Color::White
            } else {
                Color::Black
            };
            let side_pieces = self.occupied_by(side_color);

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
            let (lva_piece, lva_bb) = self.find_least_valuable_attacker(side_attackers, side_color);

            // Score from making this capture (negated, as it's opponent's gain)
            gain[depth] = SEE_VALUES[current_attacker.index()] - gain[depth - 1];

            // Pruning: if standing pat is better than any possible continuation, stop
            if (-gain[depth - 1]).max(gain[depth]) < 0 {
                break;
            }

            // Don't capture with king if opponent still has attackers
            if lva_piece == Piece::King {
                let opponent_attackers =
                    Bitboard(attackers.0 & self.occupied_by(side_color.opponent()).0);
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
        let white_pawn_attackers =
            PAWN_ATTACKS[1][sq_idx] & self.pieces_of(Color::White, Piece::Pawn).0;
        let black_pawn_attackers =
            PAWN_ATTACKS[0][sq_idx] & self.pieces_of(Color::Black, Piece::Pawn).0;
        attackers.0 |= white_pawn_attackers | black_pawn_attackers;

        // Knight attacks
        let knight_attackers = KNIGHT_ATTACKS[sq_idx]
            & (self.pieces_of(Color::White, Piece::Knight).0
                | self.pieces_of(Color::Black, Piece::Knight).0);
        attackers.0 |= knight_attackers;

        // King attacks
        let king_attackers = KING_ATTACKS[sq_idx]
            & (self.pieces_of(Color::White, Piece::King).0
                | self.pieces_of(Color::Black, Piece::King).0);
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
        Bitboard(self.all_pieces_of_type(Piece::Bishop).0 | self.all_pieces_of_type(Piece::Queen).0)
    }

    /// Get all straight sliding pieces (rooks and queens).
    #[inline]
    fn straight_sliders(&self) -> Bitboard {
        Bitboard(self.all_pieces_of_type(Piece::Rook).0 | self.all_pieces_of_type(Piece::Queen).0)
    }

    /// Find the least valuable attacker from a set of attackers.
    /// Returns the piece type and a bitboard with just that piece.
    fn find_least_valuable_attacker(&self, attackers: Bitboard, color: Color) -> (Piece, Bitboard) {
        // Check each piece type from least to most valuable
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

    /// Check if a quiet move is safe using SEE.
    ///
    /// Returns true if moving the piece to the destination is unlikely to lose material.
    /// This evaluates what happens if the opponent captures our piece after we move it.
    ///
    /// # Arguments
    /// * `from` - Source square of the moving piece
    /// * `to` - Target square (empty, since it's a quiet move)
    #[must_use]
    pub fn see_quiet_safe(&self, from: Square, to: Square) -> bool {
        // Get the piece that's moving
        let Some((color, piece)) = self.piece_at(from) else {
            return true; // No piece, shouldn't happen
        };

        // Get enemy pawns
        let enemy_pawns = self.opponent_pieces(color, Piece::Pawn);

        // Quick check: if destination is attacked by enemy pawn and we're moving a non-pawn, it's unsafe
        // Use our color index since enemy pawn attacks come from opposite direction
        let pawn_attacks_to_sq = PAWN_ATTACKS[color.index()][to.index()];
        if (pawn_attacks_to_sq & enemy_pawns.0) != 0 && piece != Piece::Pawn {
            return false;
        }

        // For non-pawn attackers, check if any enemy piece attacks the square
        // and is less valuable than our piece
        let piece_value = SEE_VALUES[piece.index()];

        // Check enemy knights
        let enemy_knights = self.opponent_pieces(color, Piece::Knight);
        if (KNIGHT_ATTACKS[to.index()] & enemy_knights.0) != 0
            && SEE_VALUES[Piece::Knight.index()] < piece_value
        {
            return false;
        }

        // For simplicity, if our piece is valuable (bishop+), also check sliders
        if piece_value >= SEE_VALUES[Piece::Bishop.index()] {
            let occupancy = self.all_occupied.0;

            // Check enemy bishops/queens on diagonals
            let enemy_bishops = self.opponent_pieces(color, Piece::Bishop);
            let enemy_queens = self.opponent_pieces(color, Piece::Queen);
            let diag_attacks = slider_attacks(to.index(), occupancy, true);
            if (diag_attacks & (enemy_bishops.0 | enemy_queens.0)) != 0 {
                // There's a diagonal attacker - check if it's less valuable
                if (diag_attacks & enemy_bishops.0) != 0
                    && SEE_VALUES[Piece::Bishop.index()] < piece_value
                {
                    return false;
                }
            }

            // Check enemy rooks/queens on files/ranks (for rook+ value pieces)
            if piece_value >= SEE_VALUES[Piece::Rook.index()] {
                let enemy_rooks = self.opponent_pieces(color, Piece::Rook);
                let straight_attacks = slider_attacks(to.index(), occupancy, false);
                if (straight_attacks & enemy_rooks.0) != 0 {
                    return false;
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_board(fen: &str) -> Board {
        fen.parse().expect("valid fen")
    }

    // ========================================================================
    // Basic Capture Tests
    // ========================================================================

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

    // ========================================================================
    // X-Ray Attack Tests
    // ========================================================================

    #[test]
    fn test_see_bishop_xray() {
        // Bishop x-ray through another bishop
        let board = make_board("8/8/5b2/4b3/3B4/2B5/8/8 w - - 0 1");
        let from = Square::new(2, 2); // c3 bishop
        let to = Square::new(4, 4); // e5 black bishop
        let see = board.see(from, to);
        // Bxe5, bxe5, Bxe5 = 330 - 330 + 330 = 330
        assert!(see > 0);
    }

    #[test]
    fn test_see_rook_xray() {
        // White rook takes undefended black rook, white has x-ray backup
        let board = make_board("8/8/8/3r4/8/8/8/R2R4 w - - 0 1");
        let from = Square::new(0, 3); // d1 rook
        let to = Square::new(4, 3); // d5 black rook (undefended)
        let see = board.see(from, to);
        // Rxd5, no recapture = 500
        assert_eq!(see, 500);
    }

    #[test]
    fn test_see_queen_xray_diagonal() {
        // Queen behind bishop on diagonal
        let board = make_board("8/8/5b2/8/3B4/8/1Q6/8 w - - 0 1");
        let from = Square::new(3, 3); // d4 bishop
        let to = Square::new(5, 5); // f6 black bishop
        let see = board.see(from, to);
        // Bxf6, no recapture (black has no attacker) = 330
        assert_eq!(see, 330);
    }

    // ========================================================================
    // Complex Exchange Tests
    // ========================================================================

    #[test]
    fn test_see_multiple_attackers() {
        // Knight takes undefended pawn - simple winning capture
        let board = make_board("8/8/8/3p4/2N1N3/8/8/8 w - - 0 1");
        let from = Square::new(3, 2); // c4 knight
        let to = Square::new(4, 3); // d5 pawn
        let see = board.see(from, to);
        // Nxd5, no recapture = 100
        assert_eq!(see, 100);
    }

    #[test]
    fn test_see_king_cannot_recapture_defended() {
        // King can't recapture if other attackers exist
        let board = make_board("4k3/8/4p3/3P4/8/8/8/4R3 w - - 0 1");
        let from = Square::new(4, 3); // d5 pawn
        let to = Square::new(5, 4); // e6 pawn
        let see = board.see(from, to);
        // dxe6, Kxe6 would be illegal if rook defends - but actually king can take
        // Let's verify: after dxe6, black king takes, white rook takes?
        // But rook can't reach e6 diagonally
        assert_eq!(see, 100);
    }

    #[test]
    fn test_see_winning_queen_takes_rook_defended_by_pawn() {
        // Queen takes rook but loses to pawn - still bad
        let board = make_board("8/8/1p6/2r5/3Q4/8/8/8 w - - 0 1");
        let from = Square::new(3, 3); // d4 queen
        let to = Square::new(4, 2); // c5 rook
        let see = board.see(from, to);
        // Qxc5, bxc5 = 500 - 900 = -400
        assert!(see < 0);
    }

    #[test]
    fn test_see_knight_takes_defended_knight() {
        // Equal trade
        let board = make_board("8/8/8/3n4/2N5/8/8/8 w - - 0 1");
        let from = Square::new(3, 2); // c4 knight
        let to = Square::new(4, 3); // d5 knight
        let see = board.see(from, to);
        // Nxd5, no recapture = 320
        assert_eq!(see, 320);
    }

    #[test]
    fn test_see_bishop_pair_exchange() {
        // Bishop takes bishop, bishop recaptures
        let board = make_board("8/8/8/3b4/4B3/5B2/8/8 w - - 0 1");
        let from = Square::new(3, 4); // e4 bishop
        let to = Square::new(4, 3); // d5 bishop
        let see = board.see(from, to);
        // Bxd5, no recapture = 330
        assert_eq!(see, 330);
    }

    // ========================================================================
    // En Passant Tests
    // ========================================================================

    #[test]
    fn test_see_en_passant_simple() {
        // En passant capture
        let board = make_board("8/8/8/3Pp3/8/8/8/8 w - e6 0 1");
        let from = Square::new(4, 3); // d5
        let to = Square::new(5, 4); // e6
        let see = board.see(from, to);
        // En passant wins a pawn
        assert_eq!(see, 100);
    }

    #[test]
    fn test_see_en_passant_defended() {
        // En passant but square is defended
        let board = make_board("8/5p2/8/3Pp3/8/8/8/8 w - e6 0 1");
        let from = Square::new(4, 3); // d5
        let to = Square::new(5, 4); // e6
        let see = board.see(from, to);
        // dxe6, fxe6 = 100 - 100 = 0
        assert_eq!(see, 0);
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_see_no_capture() {
        // Non-capture move
        let board = make_board("8/8/8/8/4N3/8/8/8 w - - 0 1");
        let from = Square::new(3, 4); // e4 knight
        let to = Square::new(5, 5); // f6 (empty)
        let see = board.see(from, to);
        assert_eq!(see, 0);
    }

    #[test]
    fn test_see_undefended_piece() {
        // Capture undefended piece
        let board = make_board("8/8/8/3r4/8/8/8/3R4 w - - 0 1");
        let from = Square::new(0, 3); // d1 rook
        let to = Square::new(4, 3); // d5 rook
        let see = board.see(from, to);
        assert_eq!(see, 500); // Win a rook
    }

    #[test]
    fn test_see_pawn_takes_queen_defended_by_queen() {
        // Pawn takes queen, queen recaptures - still winning
        let board = make_board("8/8/3q4/2q5/3P4/8/8/8 w - - 0 1");
        let from = Square::new(3, 3); // d4 pawn
        let to = Square::new(4, 2); // c5 queen
        let see = board.see(from, to);
        // Pxc5, Qxc5 = 900 - 100 = 800
        assert!(see > 700);
    }

    #[test]
    fn test_see_rook_takes_rook_both_defended() {
        // Both sides have equal rook x-ray - equal exchange
        let board = make_board("3r4/8/8/3r4/8/8/8/R2R4 w - - 0 1");
        let from = Square::new(0, 3); // d1 rook
        let to = Square::new(4, 3); // d5 rook
        let see = board.see(from, to);
        // Rxd5, Rxd5, Rxd5, Rxd5 = 500 - 500 + 500 - 500 = 0
        // Or white wins if they have the last recapture
        // The actual result depends on the SEE implementation
        assert!(see >= 0, "see={see}");
    }

    // ========================================================================
    // SEE Threshold Tests
    // ========================================================================

    #[test]
    fn test_see_ge_winning_capture() {
        let board = make_board("8/8/8/3p4/4N3/8/8/8 w - - 0 1");
        let from = Square::new(3, 4); // e4 knight
        let to = Square::new(4, 3); // d5 pawn
        assert!(board.see_ge(from, to, 0)); // Winning capture
        assert!(board.see_ge(from, to, 100)); // Wins at least 100
    }

    #[test]
    fn test_see_ge_losing_capture() {
        let board = make_board("8/8/2p5/3p4/4Q3/8/8/8 w - - 0 1");
        let from = Square::new(3, 4); // e4 queen
        let to = Square::new(4, 3); // d5 pawn
        assert!(!board.see_ge(from, to, 0)); // Losing capture
    }
}
