use crate::bitboards::BitBoard;
use once_cell::sync::Lazy;

// Pre-computed attack tables for maximum speed
pub struct AttackTables {
    pub knight_attacks: [BitBoard; 64],
    pub king_attacks: [BitBoard; 64],
    pub pawn_attacks: [[BitBoard; 64]; 2], // [color][square]
    // Magic bitboard tables for sliding pieces
    pub rook_magics: [MagicEntry; 64],
    pub bishop_magics: [MagicEntry; 64],
    pub rook_attacks: Vec<BitBoard>,
    pub bishop_attacks: Vec<BitBoard>,
}

#[derive(Clone, Copy)]
pub struct MagicEntry {
    pub mask: BitBoard,
    pub magic: u64,
    pub shift: u32,
    pub offset: usize,
}

impl AttackTables {
    fn new() -> Self {
        let mut tables = AttackTables {
            knight_attacks: [0; 64],
            king_attacks: [0; 64],
            pawn_attacks: [[0; 64]; 2],
            rook_magics: [MagicEntry { mask: 0, magic: 0, shift: 0, offset: 0 }; 64],
            bishop_magics: [MagicEntry { mask: 0, magic: 0, shift: 0, offset: 0 }; 64],
            rook_attacks: Vec::new(),
            bishop_attacks: Vec::new(),
        };
        
        tables.init_knight_attacks();
        tables.init_king_attacks();
        tables.init_pawn_attacks();
        tables.init_sliding_attacks();
        tables
    }
    
    fn init_knight_attacks(&mut self) {
        const KNIGHT_DELTAS: [(i8, i8); 8] = [
            (2, 1), (1, 2), (-1, 2), (-2, 1), 
            (-2, -1), (-1, -2), (1, -2), (2, -1),
        ];
        
        for sq in 0..64 {
            let rank = sq / 8;
            let file = sq % 8;
            let mut attacks = 0u64;
            
            for &(dr, df) in &KNIGHT_DELTAS {
                let new_rank = rank as i8 + dr;
                let new_file = file as i8 + df;
                
                if new_rank >= 0 && new_rank < 8 && new_file >= 0 && new_file < 8 {
                    attacks |= 1u64 << (new_rank * 8 + new_file);
                }
            }
            self.knight_attacks[sq] = attacks;
        }
    }
    
    fn init_king_attacks(&mut self) {
        const KING_DELTAS: [(i8, i8); 8] = [
            (1, 0), (-1, 0), (0, 1), (0, -1),
            (1, 1), (1, -1), (-1, 1), (-1, -1),
        ];
        
        for sq in 0..64 {
            let rank = sq / 8;
            let file = sq % 8;
            let mut attacks = 0u64;
            
            for &(dr, df) in &KING_DELTAS {
                let new_rank = rank as i8 + dr;
                let new_file = file as i8 + df;
                
                if new_rank >= 0 && new_rank < 8 && new_file >= 0 && new_file < 8 {
                    attacks |= 1u64 << (new_rank * 8 + new_file);
                }
            }
            self.king_attacks[sq] = attacks;
        }
    }
    
    fn init_pawn_attacks(&mut self) {
        for sq in 0..64 {
            let rank = sq / 8;
            let file = sq % 8;
            
            // White pawn attacks
            if rank < 7 {
                if file > 0 {
                    self.pawn_attacks[0][sq] |= 1u64 << ((rank + 1) * 8 + file - 1);
                }
                if file < 7 {
                    self.pawn_attacks[0][sq] |= 1u64 << ((rank + 1) * 8 + file + 1);
                }
            }
            
            // Black pawn attacks
            if rank > 0 {
                if file > 0 {
                    self.pawn_attacks[1][sq] |= 1u64 << ((rank - 1) * 8 + file - 1);
                }
                if file < 7 {
                    self.pawn_attacks[1][sq] |= 1u64 << ((rank - 1) * 8 + file + 1);
                }
            }
        }
    }
    
    fn init_sliding_attacks(&mut self) {
        // Simplified magic bitboard initialization
        // In a full implementation, you'd use proper magic numbers
        // This is a placeholder that uses perfect hashing
        
        let mut rook_offset = 0;
        let mut bishop_offset = 0;
        
        // Pre-allocate attack tables
        self.rook_attacks.resize(102400, 0); 
        self.bishop_attacks.resize(5248, 0);
        
        for sq in 0..64 {
            // Rook attacks
            let rook_mask = self.generate_rook_mask(sq);
            let rook_bits = rook_mask.count_ones();
            
            self.rook_magics[sq] = MagicEntry {
                mask: rook_mask,
                magic: ROOK_MAGICS[sq],
                shift: 64 - rook_bits,
                offset: rook_offset,
            };
            
            // Generate all possible occupancy combinations
            let combinations = 1u64 << rook_bits;
            for i in 0..combinations {
                let occupancy = self.index_to_occupancy(i, rook_bits, rook_mask);
                let attacks = self.generate_rook_attacks_slow(sq, occupancy);
                let index = self.occupancy_to_index(occupancy & rook_mask, rook_mask) as usize;
                self.rook_attacks[rook_offset + index] = attacks;
            }
            rook_offset += combinations as usize;
            
            // Similar for bishops
            let bishop_mask = self.generate_bishop_mask(sq);
            let bishop_bits = bishop_mask.count_ones();
            
            self.bishop_magics[sq] = MagicEntry {
                mask: bishop_mask,
                magic: BISHOP_MAGICS[sq],
                shift: 64 - bishop_bits,
                offset: bishop_offset,
            };
            
            let combinations = 1u64 << bishop_bits;
            for i in 0..combinations {
                let occupancy = self.index_to_occupancy(i, bishop_bits, bishop_mask);
                let attacks = self.generate_bishop_attacks_slow(sq, occupancy);
                let index = self.occupancy_to_index(occupancy & bishop_mask, bishop_mask) as usize;
                self.bishop_attacks[bishop_offset + index] = attacks;
            }
            bishop_offset += combinations as usize;
        }
    }
    
    // Helper methods for magic bitboard generation
    fn generate_rook_mask(&self, sq: usize) -> BitBoard {
        let rank = sq / 8;
        let file = sq % 8;
        let mut mask = 0u64;
        
        // Horizontal
        for f in 1..7 {
            if f != file {
                mask |= 1u64 << (rank * 8 + f);
            }
        }
        
        // Vertical  
        for r in 1..7 {
            if r != rank {
                mask |= 1u64 << (r * 8 + file);
            }
        }
        
        mask
    }
    
    fn generate_bishop_mask(&self, sq: usize) -> BitBoard {
        let rank = sq / 8;
        let file = sq % 8;
        let mut mask = 0u64;
        
        // All four diagonal directions, excluding edges
        let directions = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
        
        for &(dr, df) in &directions {
            let mut r = rank as i32 + dr;
            let mut f = file as i32 + df;
            
            while r >= 1 && r <= 6 && f >= 1 && f <= 6 {
                mask |= 1u64 << (r * 8 + f);
                r += dr;
                f += df;
            }
        }
        
        mask
    }
    
    fn generate_rook_attacks_slow(&self, sq: usize, occupancy: BitBoard) -> BitBoard {
        let rank = sq / 8;
        let file = sq % 8;
        let mut attacks = 0u64;
        
        // Right
        for f in (file + 1)..8 {
            attacks |= 1u64 << (rank * 8 + f);
            if (occupancy & (1u64 << (rank * 8 + f))) != 0 { break; }
        }
        
        // Left
        for f in (0..file).rev() {
            attacks |= 1u64 << (rank * 8 + f);
            if (occupancy & (1u64 << (rank * 8 + f))) != 0 { break; }
        }
        
        // Up
        for r in (rank + 1)..8 {
            attacks |= 1u64 << (r * 8 + file);
            if (occupancy & (1u64 << (r * 8 + file))) != 0 { break; }
        }
        
        // Down
        for r in (0..rank).rev() {
            attacks |= 1u64 << (r * 8 + file);
            if (occupancy & (1u64 << (r * 8 + file))) != 0 { break; }
        }
        
        attacks
    }
    
    fn generate_bishop_attacks_slow(&self, sq: usize, occupancy: BitBoard) -> BitBoard {
        let rank = sq / 8;
        let file = sq % 8;
        let mut attacks = 0u64;
        let directions = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
        
        for &(dr, df) in &directions {
            let mut r = rank as i32 + dr;
            let mut f = file as i32 + df;
            
            while r >= 0 && r < 8 && f >= 0 && f < 8 {
                attacks |= 1u64 << (r * 8 + f);
                if (occupancy & (1u64 << (r * 8 + f))) != 0 { break; }
                r += dr;
                f += df;
            }
        }
        
        attacks
    }
    
    fn index_to_occupancy(&self, index: u64, bits: u32, mask: BitBoard) -> BitBoard {
        let mut occupancy = 0u64;
        let mut temp_mask = mask;
        
        for i in 0..bits {
            let square = temp_mask.trailing_zeros();
            temp_mask &= temp_mask - 1; // Clear LSB
            
            if (index & (1u64 << i)) != 0 {
                occupancy |= 1u64 << square;
            }
        }
        
        occupancy
    }
    
    fn magic_hash(&self, occupancy: BitBoard, magic: u64, shift: u32) -> usize {
        ((occupancy.wrapping_mul(magic)) >> shift) as usize
    }

    // Deterministic mapping from occupancy restricted to mask -> [0, 2^bits)
    fn occupancy_to_index(&self, occupancy: BitBoard, mask: BitBoard) -> u64 {
        let mut index = 0u64;
        let mut bit = 0u64;
        let mut m = mask;
        while m != 0 {
            let sq = m.trailing_zeros() as usize;
            m &= m - 1;
            if (occupancy & (1u64 << sq)) != 0 { index |= 1u64 << bit; }
            bit += 1;
        }
        index
    }
}

// Fast lookup functions
impl AttackTables {
    #[inline(always)]
    pub fn knight_attacks(&self, sq: usize) -> BitBoard {
        self.knight_attacks[sq]
    }
    
    #[inline(always)]
    pub fn king_attacks(&self, sq: usize) -> BitBoard {
        self.king_attacks[sq]
    }
    
    #[inline(always)]
    pub fn pawn_attacks(&self, sq: usize, color: usize) -> BitBoard {
        self.pawn_attacks[color][sq]
    }
    
    #[inline(always)]
    pub fn rook_attacks(&self, sq: usize, occupancy: BitBoard) -> BitBoard {
        let magic = &self.rook_magics[sq];
        let occ = occupancy & magic.mask;
        let index = self.occupancy_to_index(occ, magic.mask) as usize;
        self.rook_attacks[magic.offset + index]
    }
    
    #[inline(always)]
    pub fn bishop_attacks(&self, sq: usize, occupancy: BitBoard) -> BitBoard {
        let magic = &self.bishop_magics[sq];
        let occ = occupancy & magic.mask;
        let index = self.occupancy_to_index(occ, magic.mask) as usize;
        self.bishop_attacks[magic.offset + index]
    }
    
    #[inline(always)]
    pub fn queen_attacks(&self, sq: usize, occupancy: BitBoard) -> BitBoard {
        self.rook_attacks(sq, occupancy) | self.bishop_attacks(sq, occupancy)
    }
}

static ATTACK_TABLES: Lazy<AttackTables> = Lazy::new(AttackTables::new);

pub fn get_attack_tables() -> &'static AttackTables {
    &ATTACK_TABLES
}

// Magic numbers (simplified - use proper magic numbers in production)
const ROOK_MAGICS: [u64; 64] = [
    0x8a80104000800020, 0x140002000100040, 0x2801880a0017001, 0x100081001000420,
    0x200020010080420, 0x3001c0002010008, 0x8480008002000100, 0x2080088004402900,
    0x800098204000, 0x2024401000200040, 0x100802000801000, 0x120800800801000,
    0x208808088000400, 0x2802200800400, 0x2200800100020080, 0x801000060821100,
    0x80044006422000, 0x100808020004000, 0x12108a0010204200, 0x140848010000802,
    0x481828014002800, 0x8094004002004100, 0x4010040010010802, 0x20008806104,
    0x100400080208000, 0x2040002120081000, 0x21200680100081, 0x20100080080080,
    0x2000a00200410, 0x20080800400, 0x80088400100102, 0x80004600042881,
    0x4040008040800020, 0x440003000200801, 0x4200011004500, 0x188020010100100,
    0x14800401802800, 0x2080040080800200, 0x124080204001001, 0x200046502000484,
    0x480400080088020, 0x1000422010034000, 0x30200100110040, 0x100021010009,
    0x2002080100110004, 0x202008004008002, 0x20020004010100, 0x2048440040820001,
    0x101002200408200, 0x40802000401080, 0x4008142004410100, 0x2060820c0120200,
    0x1001004080100, 0x20c020080040080, 0x2935610830022400, 0x44440041009200,
    0x280001040802101, 0x2100190040002085, 0x80c0084100102001, 0x4024081001000421,
    0x20030a0244872, 0x12001008414402, 0x2006104900a0804, 0x1004081002402,
];

const BISHOP_MAGICS: [u64; 64] = [
    0x40040844404084, 0x2004208a004208, 0x10190041080202, 0x108060845042010,
    0x581104180800210, 0x2112080446200010, 0x1080820820060210, 0x3c0808410220200,
    0x4050404440404, 0x21001420088, 0x24d0080801082102, 0x1020a0a020400,
    0x40308200402, 0x4011002100800, 0x401484104104005, 0x801010402020200,
    0x400210c3880100, 0x404022024108200, 0x810018200204102, 0x4002801a02003,
    0x85040820080400, 0x810102c808880400, 0x2002410405006000, 0x402010408800804,
    0x420a408b040400, 0x2040050880800400, 0x401002010404003, 0x40c0104080004,
    0x204000404000801, 0x44008008008020, 0x404010044040200, 0x48200040001,
    0x2000040402000a2, 0x10800040001000, 0x8008020008004, 0x1020005000800,
    0x10201008080404, 0x40080a0020400, 0x820080041001000, 0x8008080200001,
    0x1000404020010000, 0x4004080020002000, 0x10080420004004, 0x20001008104,
    0x4000205a00004100, 0x800400204800, 0x420000a040020, 0x1000801004004,
    0x821002001001040, 0x5408004008080, 0x2000a040080080, 0x40002048004004,
    0xa40001008080, 0x2000402020020040, 0x8408080020004, 0x84040090004001,
    0x200048040200800, 0x9004004080004, 0x1020000808402, 0x404400aa008800,
    0x2080200080080, 0x20004008004, 0x2200208004000, 0x1000808004000,
];