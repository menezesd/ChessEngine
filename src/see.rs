use crate::board::Board;
use crate::types::{Move, Piece, Square};

// Piece values in centipawns
const VALUES: [i32; 6] = [100, 325, 325, 500, 975, 20000];

fn piece_value(p: Piece) -> i32 {
    VALUES[p as usize]
}

// Helper: get least significant bit index
fn lsb_index(bb: u64) -> Option<usize> {
    if bb == 0 { None } else { Some(bb.trailing_zeros() as usize) }
}

/// Compute attackers to `sq` for `side_index` (0=White,1=Black) given current
/// piece bitboards and occupancy.
fn attackers_to_square(
    sq: Square,
    occ: u64,
    pieces: &[[u64; 6]; 2],
    side_index: usize,
) -> u64 {
    use crate::types::square_index;
    use crate::board::Board as B;

    let mut atk = 0u64;
    let sq_idx = square_index(sq);
    // let mask = 1u64 << sq_idx; // unused

    // Pawns: compute pawn source squares that attack sq
    let file = sq.1;
    // white pawn sources are at sq-9 and sq-7
    if side_index == 0 {
        if sq_idx >= 9 && file != 0 {
            atk |= pieces[0][0] & (1u64 << (sq_idx - 9));
        }
        if sq_idx >= 7 && file != 7 {
            atk |= pieces[0][0] & (1u64 << (sq_idx - 7));
        }
    } else {
        // black pawn sources are at sq+9 and sq+7
        if sq_idx + 9 < 64 && file != 7 {
            atk |= pieces[1][0] & (1u64 << (sq_idx + 9));
        }
        if sq_idx + 7 < 64 && file != 0 {
            atk |= pieces[1][0] & (1u64 << (sq_idx + 7));
        }
    }

    // Knights
    let knight_bb = pieces[side_index][1];
    atk |= knight_bb & B::knight_attacks(sq);

    // King (rare in swap-off, but include)
    let king_bb = pieces[side_index][5];
    atk |= king_bb & B::king_attacks(sq);

    // Sliding pieces: bishops/queens on bishop rays
    let bishop_like = pieces[side_index][2] | pieces[side_index][4];
    let bishop_attacks = B::bishop_attacks(sq, occ);
    atk |= bishop_like & bishop_attacks;

    // Rooks/queens on rook rays
    let rook_like = pieces[side_index][3] | pieces[side_index][4];
    let rook_attacks = B::rook_attacks(sq, occ);
    atk |= rook_like & rook_attacks;

    atk
}

/// Full static exchange evaluator (swap-off) for capture `mv` on `board`.
/// Returns positive centipawns if capture sequence favors the attacker.
pub fn see_capture(board: &Board, mv: &Move) -> i32 {
    // Only defined for captures
    let captured_piece = match mv.captured_piece {
        Some(p) => p,
        None => return 0,
    };

    use crate::types::square_index;

    // Clone bitboards and occupancy to simulate capture sequence
    let mut pieces: [[u64; 6]; 2] = board.bitboards;
    let mut occ = board.all_occupancy;

    let from_idx = square_index(mv.from);
    let to_idx = square_index(mv.to);
    let from_mask = 1u64 << from_idx;
    let to_mask = 1u64 << to_idx;

    // Determine sides
    let (att_side, def_side) = match board.piece_at(mv.from) {
        Some((c, _)) => (if c == crate::types::Color::White { 0 } else { 1 }, if c == crate::types::Color::White { 1 } else { 0 }),
        None => return 0,
    };

    // Attacker piece type index
    let attacker_piece = board.piece_at(mv.from).map(|(_c, p)| p).unwrap_or(Piece::Pawn);
    let attacker_idx = attacker_piece as usize;
    let captured_idx = captured_piece as usize;

    // Perform the initial capture on the simulation boards:
    // remove attacker from from, remove captured from to, place attacker at to
    pieces[att_side][attacker_idx] &= !from_mask;
    occ &= !from_mask;
    // remove victim
    pieces[def_side][captured_idx] &= !to_mask;
    occ &= !to_mask;
    // place attacker on to
    pieces[att_side][attacker_idx] |= to_mask;
    occ |= to_mask;

    // Gains array
    let mut gains: Vec<i32> = Vec::new();
    gains.push(piece_value(captured_piece));

    // side to move next is defender (they can recapture)
    let mut side = def_side;

    // Loop collecting possible recaptures (take least valuable attacker each time)
    loop {
        let atks = attackers_to_square(mv.to, occ, &pieces, side);
        if atks == 0 { break; }

        // Find least valuable attacker among atks
        let mut picked_sq: Option<usize> = None;
        let mut picked_val = i32::MAX;
        let mut picked_piece_idx = 0usize;

        // Check piece types in ascending value order: Pawn, Knight, Bishop, Rook, Queen, King
        for (piece_idx, &vals) in VALUES.iter().enumerate() {
            let bb = pieces[side][piece_idx] & atks;
            if bb != 0 {
                if let Some(sq) = lsb_index(bb) {
                    let val = vals;
                    if val < picked_val {
                        picked_val = val;
                        picked_sq = Some(sq);
                        picked_piece_idx = piece_idx;
                    }
                }
            }
        }

        if picked_sq.is_none() { break; }
        let sq = picked_sq.unwrap();
        let mask = 1u64 << sq;

        // Add to gains: piece value - previous gain
        gains.push(VALUES[picked_piece_idx] - gains[gains.len() - 1]);

        // Remove the picked attacker from simulation
        pieces[side][picked_piece_idx] &= !mask;
        occ &= !mask;

        // If the picked attacker is a sliding piece, removing it may open new attacks.
        // The loop recomputes attackers_to_square each iteration using updated occ.

        side = 1 - side; // switch side
    }

    // Now solve the minimax-like sequence backwards
    for i in (0..gains.len()-1).rev() {
        gains[i] = gains[i].max(-gains[i+1]);
    }

    gains[0]
}
