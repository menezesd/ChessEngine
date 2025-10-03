// Fast bitboard utilities for maximum performance
use crate::bitboards::BitBoard;

// Precomputed lookup tables for faster bit operations
pub struct BitboardUtils {
    pub square_masks: [BitBoard; 64],
    pub file_masks: [BitBoard; 8],
    pub rank_masks: [BitBoard; 8],
    pub diagonal_masks: [BitBoard; 15],
    pub anti_diagonal_masks: [BitBoard; 15],
    pub between_masks: [[BitBoard; 64]; 64],
    pub ray_masks: [[BitBoard; 64]; 64],
}

impl BitboardUtils {
    pub fn new() -> Self {
        let mut utils = BitboardUtils {
            square_masks: [0; 64],
            file_masks: [0; 8],
            rank_masks: [0; 8],
            diagonal_masks: [0; 15],
            anti_diagonal_masks: [0; 15],
            between_masks: [[0; 64]; 64],
            ray_masks: [[0; 64]; 64],
        };
        
        utils.init_square_masks();
        utils.init_file_rank_masks();
        utils.init_diagonal_masks();
        utils.init_between_and_ray_masks();
        
        utils
    }
    
    fn init_square_masks(&mut self) {
        for sq in 0..64 {
            self.square_masks[sq] = 1u64 << sq;
        }
    }
    
    fn init_file_rank_masks(&mut self) {
        for file in 0..8 {
            self.file_masks[file] = 0x0101010101010101u64 << file;
        }
        
        for rank in 0..8 {
            self.rank_masks[rank] = 0xFFu64 << (rank * 8);
        }
    }
    
    fn init_diagonal_masks(&mut self) {
        // Main diagonals (a1-h8 direction)
        for diag in 0..15 {
            let mut mask = 0u64;
            let start_file = if diag <= 7 { 0 } else { diag - 7 };
            let start_rank = if diag <= 7 { 7 - diag } else { 0 };
            
            let mut file = start_file;
            let mut rank = start_rank;
            
            while file < 8 && rank < 8 {
                mask |= 1u64 << (rank * 8 + file);
                file += 1;
                rank += 1;
            }
            
            self.diagonal_masks[diag] = mask;
        }
        
        // Anti-diagonals (a8-h1 direction)
        for diag in 0..15 {
            let mut mask = 0u64;
            let start_file = if diag <= 7 { 0 } else { diag - 7 };
            let start_rank = if diag <= 7 { diag } else { 7 };
            
            let mut file = start_file;
            let mut rank = start_rank as i32;
            
            while file < 8 && rank >= 0 {
                mask |= 1u64 << (rank * 8 + file as i32);
                file += 1;
                rank -= 1;
            }
            
            self.anti_diagonal_masks[diag] = mask;
        }
    }
    
    fn init_between_and_ray_masks(&mut self) {
        for sq1 in 0..64 {
            for sq2 in 0..64 {
                if sq1 == sq2 {
                    continue;
                }
                
                let rank1 = sq1 / 8;
                let file1 = sq1 % 8;
                let rank2 = sq2 / 8;
                let file2 = sq2 % 8;
                
                let rank_diff = rank2 as i32 - rank1 as i32;
                let file_diff = file2 as i32 - file1 as i32;
                
                // Check if squares are on same rank, file, or diagonal
                if rank_diff == 0 || file_diff == 0 || rank_diff.abs() == file_diff.abs() {
                    let mut between_mask = 0u64;
                    let mut ray_mask = 0u64;
                    
                    let step_rank = rank_diff.signum();
                    let step_file = file_diff.signum();
                    
                    // Between mask (excluding endpoints)
                    let mut r = rank1 as i32 + step_rank;
                    let mut f = file1 as i32 + step_file;
                    
                    while r != rank2 as i32 || f != file2 as i32 {
                        between_mask |= 1u64 << (r * 8 + f);
                        r += step_rank;
                        f += step_file;
                    }
                    
                    // Ray mask (from sq1 through sq2 and beyond)
                    r = rank1 as i32;
                    f = file1 as i32;
                    
                    loop {
                        r += step_rank;
                        f += step_file;
                        
                        if r < 0 || r >= 8 || f < 0 || f >= 8 {
                            break;
                        }
                        
                        ray_mask |= 1u64 << (r * 8 + f);
                    }
                    
                    self.between_masks[sq1][sq2] = between_mask;
                    self.ray_masks[sq1][sq2] = ray_mask;
                }
            }
        }
    }
}

use once_cell::sync::Lazy;
static BITBOARD_UTILS: Lazy<BitboardUtils> = Lazy::new(BitboardUtils::new);

pub fn get_bitboard_utils() -> &'static BitboardUtils {
    &BITBOARD_UTILS
}

// Fast bitboard operations
#[inline(always)]
pub fn pop_lsb(bb: &mut BitBoard) -> Option<usize> {
    if *bb == 0 {
        None
    } else {
        let lsb = (*bb).trailing_zeros() as usize;
        *bb &= *bb - 1; // Clear LSB
        Some(lsb)
    }
}

#[inline(always)]
pub fn lsb(bb: BitBoard) -> Option<usize> {
    if bb == 0 {
        None
    } else {
        Some(bb.trailing_zeros() as usize)
    }
}

#[inline(always)]
pub fn msb(bb: BitBoard) -> Option<usize> {
    if bb == 0 {
        None
    } else {
        Some(63 - bb.leading_zeros() as usize)
    }
}

#[inline(always)]
pub fn pop_count(bb: BitBoard) -> u32 {
    bb.count_ones()
}

// Fast square distance calculation
#[inline(always)]
pub fn square_distance(sq1: usize, sq2: usize) -> usize {
    let rank1 = sq1 / 8;
    let file1 = sq1 % 8;
    let rank2 = sq2 / 8;
    let file2 = sq2 % 8;
    
    let rank_diff = (rank1 as i32 - rank2 as i32).abs() as usize;
    let file_diff = (file1 as i32 - file2 as i32).abs() as usize;
    
    rank_diff.max(file_diff)
}

// Check if there are pieces between two squares
#[inline(always)]
pub fn is_clear_between(sq1: usize, sq2: usize, occupancy: BitBoard) -> bool {
    let between = get_bitboard_utils().between_masks[sq1][sq2];
    (between & occupancy) == 0
}

// Get all squares in a ray from sq1 through sq2
#[inline(always)]
pub fn get_ray(sq1: usize, sq2: usize) -> BitBoard {
    get_bitboard_utils().ray_masks[sq1][sq2]
}

// Fast file/rank/diagonal getters
#[inline(always)]
pub fn get_file_mask(file: usize) -> BitBoard {
    get_bitboard_utils().file_masks[file]
}

#[inline(always)]
pub fn get_rank_mask(rank: usize) -> BitBoard {
    get_bitboard_utils().rank_masks[rank]
}

#[inline(always)]
pub fn get_square_mask(sq: usize) -> BitBoard {
    get_bitboard_utils().square_masks[sq]
}

// Pawn-specific bitboard operations
#[inline(always)]
pub fn white_pawn_pushes(pawns: BitBoard, empty: BitBoard) -> BitBoard {
    (pawns << 8) & empty
}

#[inline(always)]
pub fn black_pawn_pushes(pawns: BitBoard, empty: BitBoard) -> BitBoard {
    (pawns >> 8) & empty
}

#[inline(always)]
pub fn white_pawn_double_pushes(pawns: BitBoard, empty: BitBoard) -> BitBoard {
    const RANK_4: BitBoard = 0x00000000FF000000;
    let single_pushes = white_pawn_pushes(pawns, empty);
    (single_pushes << 8) & empty & RANK_4
}

#[inline(always)]
pub fn black_pawn_double_pushes(pawns: BitBoard, empty: BitBoard) -> BitBoard {
    const RANK_5: BitBoard = 0x000000FF00000000;
    let single_pushes = black_pawn_pushes(pawns, empty);
    (single_pushes >> 8) & empty & RANK_5
}

#[inline(always)]
pub fn white_pawn_attacks_east(pawns: BitBoard) -> BitBoard {
    const NOT_H_FILE: BitBoard = 0x7F7F7F7F7F7F7F7F;
    (pawns & NOT_H_FILE) << 9
}

#[inline(always)]
pub fn white_pawn_attacks_west(pawns: BitBoard) -> BitBoard {
    const NOT_A_FILE: BitBoard = 0xFEFEFEFEFEFEFEFE;
    (pawns & NOT_A_FILE) << 7
}

#[inline(always)]
pub fn black_pawn_attacks_east(pawns: BitBoard) -> BitBoard {
    const NOT_H_FILE: BitBoard = 0x7F7F7F7F7F7F7F7F;
    (pawns & NOT_H_FILE) >> 7
}

#[inline(always)]
pub fn black_pawn_attacks_west(pawns: BitBoard) -> BitBoard {
    const NOT_A_FILE: BitBoard = 0xFEFEFEFEFEFEFEFE;
    (pawns & NOT_A_FILE) >> 9
}

// Specialized move generation helpers
#[inline(always)]
pub fn knight_moves_from_bb(knights: BitBoard, own_pieces: BitBoard) -> BitBoard {
    let mut moves = 0u64;
    let attack_tables = crate::attack_tables::get_attack_tables();
    let mut temp = knights;
    
    while let Some(sq) = pop_lsb(&mut temp) {
        moves |= attack_tables.knight_attacks(sq);
    }
    
    moves & !own_pieces
}

#[inline(always)]
pub fn king_moves_from_bb(king: BitBoard, own_pieces: BitBoard) -> BitBoard {
    if king == 0 { return 0; }
    let sq = lsb(king).unwrap();
    let attack_tables = crate::attack_tables::get_attack_tables();
    attack_tables.king_attacks(sq) & !own_pieces
}