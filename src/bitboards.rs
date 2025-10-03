use crate::board::{Color, Square, Board};

pub type BitBoard = u64;

#[inline]
pub(crate) fn sq_to_bb(sq: Square) -> BitBoard { 1u64 << (sq.0 * 8 + sq.1) }

#[inline]
fn in_bounds(r: isize, f: isize) -> bool { r >= 0 && r < 8 && f >= 0 && f < 8 }

#[inline]
fn rf_to_sq(r: isize, f: isize) -> Square { Square(r as usize, f as usize) }

#[inline]
fn idx_to_sq(idx: usize) -> Square { Square((idx / 8) as usize, (idx % 8) as usize) }

#[inline]
fn popcount(bb: BitBoard) -> u32 { bb.count_ones() }

#[inline]
fn lsb(bb: BitBoard) -> Option<usize> {
    if bb == 0 { None } else { Some(bb.trailing_zeros() as usize) }
}

static KNIGHT_DELTAS: [(isize, isize); 8] = [
    (2, 1), (1, 2), (-1, 2), (-2, 1), (-2, -1), (-1, -2), (1, -2), (2, -1),
];

static KING_DELTAS: [(isize, isize); 8] = [
    (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1),
];

#[inline]
pub(crate) fn knight_attacks_from(sq: Square) -> BitBoard {
    let sq_idx = sq.0 * 8 + sq.1;
    crate::attack_tables::get_attack_tables().knight_attacks(sq_idx)
}

#[inline]
pub(crate) fn king_attacks_from(sq: Square) -> BitBoard {
    let sq_idx = sq.0 * 8 + sq.1;
    crate::attack_tables::get_attack_tables().king_attacks(sq_idx)
}

#[inline]
fn ray_attacks(sq: Square, occ: BitBoard, deltas: &[(isize, isize)]) -> BitBoard {
    let (r0, f0) = (sq.0 as isize, sq.1 as isize);
    let mut attacks = 0u64;
    for &(dr, df) in deltas {
        let mut r = r0 + dr; let mut f = f0 + df;
        while in_bounds(r, f) {
            let nsq = rf_to_sq(r, f);
            let nsq_bb = sq_to_bb(nsq);
            attacks |= nsq_bb;
            if (occ & nsq_bb) != 0 { break; }
            r += dr; f += df;
        }
    }
    attacks
}

#[inline]
pub(crate) fn bishop_attacks_from(sq: Square, occ: BitBoard) -> BitBoard {
    let sq_idx = sq.0 * 8 + sq.1;
    crate::attack_tables::get_attack_tables().bishop_attacks(sq_idx, occ)
}

#[inline]
pub(crate) fn rook_attacks_from(sq: Square, occ: BitBoard) -> BitBoard {
    let sq_idx = sq.0 * 8 + sq.1;
    crate::attack_tables::get_attack_tables().rook_attacks(sq_idx, occ)
}

pub fn is_square_attacked_bb(
    board: &Board,
    target: Square,
    attacker_color: Color,
) -> bool {
    // Get occupancy and attacker piece bitboards from the board
    let occ = board.all_pieces();
    let (pawns, knights, bishops, rooks, queens, king) = match attacker_color {
        Color::White => (board.white_pawns, board.white_knights, board.white_bishops, board.white_rooks, board.white_queens, board.white_king),
        Color::Black => (board.black_pawns, board.black_knights, board.black_bishops, board.black_rooks, board.black_queens, board.black_king),
    };

    let tgt_bb = sq_to_bb(target);

    // Pawn attacks - optimized with pre-computed tables
    let color_idx = if attacker_color == Color::White { 0 } else { 1 };
    let attack_tables = crate::attack_tables::get_attack_tables();
    let mut pawn_attacks = 0u64;
    let mut temp_pawns = pawns;
    while let Some(idx) = lsb(temp_pawns) {
        temp_pawns &= temp_pawns - 1; // pop lsb
        pawn_attacks |= attack_tables.pawn_attacks(idx, color_idx);
    }
    if (pawn_attacks & tgt_bb) != 0 { return true; }

    // Knight attacks
    let mut kn = knights;
    while let Some(idx) = lsb(kn) {
        kn &= kn - 1;
        if (knight_attacks_from(idx_to_sq(idx)) & tgt_bb) != 0 { return true; }
    }

    // King attacks
    if king != 0 { let idx = lsb(king).unwrap(); if (king_attacks_from(idx_to_sq(idx)) & tgt_bb) != 0 { return true; } }

    // Sliding: rook/queen orthogonals
    let mut rq = rooks | queens;
    while let Some(idx) = lsb(rq) {
        rq &= rq - 1;
        if (rook_attacks_from(idx_to_sq(idx), occ) & tgt_bb) != 0 { return true; }
    }

    // Sliding: bishop/queen diagonals
    let mut bq = bishops | queens;
    while let Some(idx) = lsb(bq) {
        bq &= bq - 1;
        if (bishop_attacks_from(idx_to_sq(idx), occ) & tgt_bb) != 0 { return true; }
    }

    false
}

// Common file/rank masks for pawn move generation (LSB = a1)
pub(crate) const FILE_A: BitBoard = 0x0101_0101_0101_0101u64;
pub(crate) const FILE_H: BitBoard = 0x8080_8080_8080_8080u64;
pub(crate) const RANK1: BitBoard = 0x0000_0000_0000_00FFu64;
pub(crate) const RANK8: BitBoard = 0xFF00_0000_0000_0000u64;

// Helpers used by movegen to detect intermediate squares for double pushes
#[inline]
pub(crate) fn rank3_mask() -> BitBoard { 0x0000_0000_0000_FF00u64 << 8 } // rank 3
#[inline]
pub(crate) fn rank6_mask() -> BitBoard { 0x00FF_0000_0000_0000u64 >> 8 } // rank 6
