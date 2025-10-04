use std::collections::HashSet;
use once_cell::sync::Lazy;
use rand::prelude::*;

use crate::uci::format_square;
use crate::TranspositionTable;

// --- Zobrist Hashing ---
struct ZobristKeys {
    piece_keys: [[[u64; 64]; 2]; 6],
    black_to_move_key: u64,
    castling_keys: [[u64; 2]; 2],
    en_passant_keys: [u64; 8],
}

impl ZobristKeys {
    fn new() -> Self {
        let mut rng = StdRng::seed_from_u64(1234567890_u64);
        let mut piece_keys = [[[0; 64]; 2]; 6];
        let mut castling_keys = [[0; 2]; 2];
        let mut en_passant_keys = [0; 8];

        for p_idx in 0..6 {
            for c_idx in 0..2 {
                for sq_idx in 0..64 {
                    piece_keys[p_idx][c_idx][sq_idx] = rng.gen();
                }
            }
        }
        let black_to_move_key = rng.gen();
        for c_idx in 0..2 { for side_idx in 0..2 { castling_keys[c_idx][side_idx] = rng.gen(); } }
        for f_idx in 0..8 { en_passant_keys[f_idx] = rng.gen(); }
        ZobristKeys { piece_keys, black_to_move_key, castling_keys, en_passant_keys }
    }
}

static ZOBRIST: Lazy<ZobristKeys> = Lazy::new(ZobristKeys::new);

// Helper to map Piece enum to index
pub(crate) fn piece_to_zobrist_index(piece: Piece) -> usize {
    match piece {
        Piece::Pawn => 0,
        Piece::Knight => 1,
        Piece::Bishop => 2,
        Piece::Rook => 3,
        Piece::Queen => 4,
        Piece::King => 5,
    }
}

// Helper to map Color enum to index
pub(crate) fn color_to_zobrist_index(color: Color) -> usize {
    match color { Color::White => 0, Color::Black => 1 }
}

// Helper to map Square to index (0-63)
pub(crate) fn square_to_zobrist_index(sq: Square) -> usize { sq.0 * 8 + sq.1 }

// --- Enums and Structs ---
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Piece { Pawn, Knight, Bishop, Rook, Queen, King }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Color { White, Black }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Square(pub usize, pub usize); // (rank, file)

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Move {
    pub(crate) from: Square,
    pub(crate) to: Square,
    pub(crate) is_castling: bool,
    pub(crate) is_en_passant: bool,
    pub(crate) promotion: Option<Piece>,
    pub(crate) captured_piece: Option<Piece>,
}

#[derive(Clone, Debug)]
pub(crate) struct UnmakeInfo {
    captured_piece_info: Option<(Color, Piece)>,
    previous_en_passant_target: Option<Square>,
    previous_castling_rights: HashSet<(Color, char)>,
    previous_hash: u64,
    previous_halfmove_clock: u32,
}

pub(crate) fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 300,
        Piece::Bishop => 300,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 10000,
    }
}

pub(crate) fn mvv_lva_score(m: &Move, board: &Board) -> i32 {
    if let Some(victim) = m.captured_piece {
        let attacker = board.piece_at(m.from).unwrap().1;
        let victim_value = piece_value(victim);
        let attacker_value = piece_value(attacker);
        victim_value * 10 - attacker_value
    } else { 0 }
}

#[derive(Clone, Debug)]
pub(crate) struct Board {
    // Bitboard representation - piece-centric
    pub(crate) white_pawns: u64,
    pub(crate) white_knights: u64,
    pub(crate) white_bishops: u64,
    pub(crate) white_rooks: u64,
    pub(crate) white_queens: u64,
    pub(crate) white_king: u64,
    pub(crate) black_pawns: u64,
    pub(crate) black_knights: u64,
    pub(crate) black_bishops: u64,
    pub(crate) black_rooks: u64,
    pub(crate) black_queens: u64,
    pub(crate) black_king: u64,
    
    pub(crate) white_to_move: bool,
    pub(crate) en_passant_target: Option<Square>,
    pub(crate) castling_rights: HashSet<(Color, char)>,
    pub(crate) hash: u64,
    pub(crate) halfmove_clock: u32,
    pub(crate) position_history: Vec<u64>,
}

// Evaluation component breakdown for instrumentation / tuning
pub struct EvalBreakdown {
        pub material: i32,
        pub piece_square: i32,
        pub mobility: i32,
        pub king_ring: i32,
        pub bishop_pair: i32,
        pub rook_files: i32,
        pub pawn_structure: i32,
        pub hanging: i32,
        pub tempo: i32,
        pub endgame_scale_applied: bool,
        pub scaled_total: i32,
        pub pre_scale_total: i32,
}

// Centralized tunable parameters
#[derive(Clone)]
pub struct EvalParams {
    pub material: [i32;6],
    pub mobility_knight: i32,
    pub mobility_bishop: i32,
    pub mobility_rook: i32,
    pub mobility_queen: i32,
    pub bishop_pair: i32,
    pub tempo: i32,
    pub isolated_pawn: i32,
    pub doubled_pawn: i32,
    pub backward_pawn: i32,
    pub passed_base: i32,
    pub passed_per_rank: i32,
    pub rook_open_file: i32,
    pub rook_semi_open: i32,
    pub hanging_penalty: i32,
    pub king_ring_attack: i32,
    pub endgame_threshold: i32,
}

impl Default for EvalParams {
    fn default() -> Self { Self {
        material: [100,325,330,500,950,0],
        mobility_knight:4, mobility_bishop:5, mobility_rook:3, mobility_queen:2,
        bishop_pair:30, tempo:8,
        isolated_pawn:15, doubled_pawn:12, backward_pawn:10,
        passed_base:12, passed_per_rank:8,
        rook_open_file:15, rook_semi_open:7,
        hanging_penalty:20, king_ring_attack:4,
        endgame_threshold:2000,
    }}
}

impl Board {
    pub fn evaluate_with_breakdown_params(&self, params: &EvalParams) -> EvalBreakdown {
        // Constants duplicated (could be DRY'd via associated consts)
        let material_arr = params.material;
        const PST: [[i32;64];6] = [
            [0,0,0,0,0,0,0,0, -5,-1,-6,-8,-3, 3, 6,-4, -8,-2,-2,-4, 1, 1, 8,-3, -6, 0,-3,-1,-1,-2, 1,-1,  6, 4, 3, 2,-2, 0, 7, 7, 30,32,27,21,18,17,26,27, 80,78,72,61,66,59,74,84, 0,0,0,0,0,0,0,0],
            [-50,-10,-40,-30,-20,-30,-10,-20,-20,-40,-10, 0, 0,15,-10,-15,-25,-5,10,10,15,10, 5,-15,-10, 5,15,10,25,15,15, 0, -5,10,15,40,25,30,10,15, -5,25,15,30,40,60,35,20, -35,-20,35,20,10,30, 5,-10,-80,-40,-15,-25,30,-50,-10,-60],
            [-20,-5,-10,-15,-10,-10,-25,-15, 5,15,15, 5,10,20,30, 0, 0,15,15,15,15,25,18,10, -5,15,15,25,30,12,10, 5, -5, 5,19,35,25,35, 7, 0,-10,37,40,38,35,45,37, 0,-15,16,-18,-13,15,30,20, 10,22,22,27,27,19,10,20, 0],
            [-10, 0, 5,10,10, 5,-20,-15,-25,-5,-10,-5, 0,10,-5,-25,-25,-10, 0,-5, 5, 0,-5,-15,-15,-10, 5,10,15,10, 5,-10,-10, 5, 5,10,15,10, 2, 0, -5,15,15,15,15,20,10, 5, 5,10,10,10,10,15, 8, 8, 5,10,10,10,10, 5, 0, 0],
            [-30,-25,-20,-40,-10,-30,-15,-30,-15,-15,-25,-15,-15,-15,-25,-20,-10,-20,10, 5,10,15, 8, 5,-10,20,15,30,25,20,25,15, 0,15,20,35,45,30,45,25, -5,10,15,35,35,25,15,10,-15,10,15,25,35,15,20, 0, 0,15,15,20,20,10,15, 0],
            [-10,30,12,-30,10,-20,25,10, 5,10, 0,-30,-20,-10, 5, 5,-10,-10,-15,-30,-30,-20,-10,-20,-30, 0,-20,-25,-30,-25,-20,-30,-15,-20,-10,-20,-20,-15,-10,-20,-10,15, 5,-5,-10, 5,15, 5, 5,-10,-5, -5, -5,-25,-20,-30, 5,-10, 2, 0,0,0,0,0]
        ];
    let (MOBILITY_KNIGHT,MOBILITY_BISHOP,MOBILITY_ROOK,MOBILITY_QUEEN) = (params.mobility_knight,params.mobility_bishop,params.mobility_rook,params.mobility_queen);
    let (BISHOP_PAIR,TEMPO) = (params.bishop_pair, params.tempo);
    let (ISOLATED_PAWN,DOUBLED_PAWN,BACKWARD_PAWN) = (params.isolated_pawn, params.doubled_pawn, params.backward_pawn);
    let (PASSED_BASE,PASSED_PER_RANK) = (params.passed_base, params.passed_per_rank);
    let (ROOK_OPEN_FILE,ROOK_SEMI_OPEN) = (params.rook_open_file, params.rook_semi_open);
    let (HANGING_PENALTY,KING_RING_ATTACK) = (params.hanging_penalty, params.king_ring_attack);

    let mut material = 0; let mut piece_square = 0; let mut mobility = 0; let mut king_ring_score = 0; let mut bishop_pair_score = 0; let mut rook_files = 0; let mut pawn_structure = 0; let mut hanging = 0; let mut tempo = 0;
        let us_white = self.white_to_move; let occ = self.all_pieces();

        // File pawn counts
        let mut white_pawns_by_file=[0u8;8]; let mut black_pawns_by_file=[0u8;8];
        let mut wp=self.white_pawns; while wp!=0 { let sq=wp.trailing_zeros() as usize; wp&=wp-1; white_pawns_by_file[sq%8]+=1; }
        let mut bp=self.black_pawns; while bp!=0 { let sq=bp.trailing_zeros() as usize; bp&=bp-1; black_pawns_by_file[sq%8]+=1; }
        let white_bishop_count = self.white_bishops.count_ones(); let black_bishop_count = self.black_bishops.count_ones();
        let w_king_sq = self.white_king.trailing_zeros() as usize; let b_king_sq = self.black_king.trailing_zeros() as usize;
        let king_ring = |sq: usize| -> u64 { crate::attack_tables::get_attack_tables().king_attacks(sq) | (1u64<<sq) };
        let w_ring = king_ring(w_king_sq); let b_ring = king_ring(b_king_sq);
        let mut white_attacks: u64 = 0; let mut black_attacks: u64 = 0;
        let piece_sets = [
            (self.white_pawns, Color::White, Piece::Pawn),(self.white_knights,Color::White,Piece::Knight),(self.white_bishops,Color::White,Piece::Bishop),(self.white_rooks,Color::White,Piece::Rook),(self.white_queens,Color::White,Piece::Queen),(self.white_king,Color::White,Piece::King),
            (self.black_pawns, Color::Black, Piece::Pawn),(self.black_knights,Color::Black,Piece::Knight),(self.black_bishops,Color::Black,Piece::Bishop),(self.black_rooks,Color::Black,Piece::Rook),(self.black_queens,Color::Black,Piece::Queen),(self.black_king,Color::Black,Piece::King),
        ];
    for (mut bb,color,piece) in piece_sets { while bb!=0 { let sq = bb.trailing_zeros() as usize; bb&=bb-1; let rank=sq/8; let file=sq%8; let mirrored=7-rank; let psq_index = if color==Color::White {rank*8+file} else {mirrored*8+file}; let base = material_arr[match piece{Piece::Pawn=>0,Piece::Knight=>1,Piece::Bishop=>2,Piece::Rook=>3,Piece::Queen=>4,Piece::King=>5}]; let psq = PST[match piece{Piece::Pawn=>0,Piece::Knight=>1,Piece::Bishop=>2,Piece::Rook=>3,Piece::Queen=>4,Piece::King=>5}][psq_index]; if color==Color::White { material+=base; piece_square+=psq; } else { material-=base; piece_square-=psq; }
            use crate::bitboards as bbm; let sq_struct = Square(rank,file); let targets:u64 = match piece { Piece::Knight=>bbm::knight_attacks_from(sq_struct) & !self.pieces_of_color(color), Piece::Bishop=>bbm::bishop_attacks_from(sq_struct, occ) & !self.pieces_of_color(color), Piece::Rook=>bbm::rook_attacks_from(sq_struct, occ) & !self.pieces_of_color(color), Piece::Queen=>(bbm::rook_attacks_from(sq_struct,occ)|bbm::bishop_attacks_from(sq_struct,occ)) & !self.pieces_of_color(color), Piece::King=>bbm::king_attacks_from(sq_struct) & !self.pieces_of_color(color), Piece::Pawn=>{ let attacks = if color==Color::White { crate::attack_tables::get_attack_tables().pawn_attacks(sq,0)} else { crate::attack_tables::get_attack_tables().pawn_attacks(sq,1)}; attacks & !self.pieces_of_color(color)} }; let mob_cnt = targets.count_ones() as i32; let mob_val = match piece { Piece::Knight=>mob_cnt*MOBILITY_KNIGHT, Piece::Bishop=>mob_cnt*MOBILITY_BISHOP, Piece::Rook=>mob_cnt*MOBILITY_ROOK, Piece::Queen=>mob_cnt*MOBILITY_QUEEN, _=>0 }; if color==Color::White { mobility+=mob_val; king_ring_score += (targets & b_ring).count_ones() as i32 * KING_RING_ATTACK; white_attacks|=targets; } else { mobility-=mob_val; king_ring_score -= (targets & w_ring).count_ones() as i32 * KING_RING_ATTACK; black_attacks|=targets; } } }
    if white_bishop_count>=2 { bishop_pair_score += BISHOP_PAIR; } if black_bishop_count>=2 { bishop_pair_score -= BISHOP_PAIR; }
        // Rook files
        let mut wr=self.white_rooks; while wr!=0 { let sq=wr.trailing_zeros() as usize; wr&=wr-1; let f=sq%8; let file_pawns = white_pawns_by_file[f]+black_pawns_by_file[f]; if file_pawns==0 { rook_files += ROOK_OPEN_FILE; } else if black_pawns_by_file[f]==0 { rook_files += ROOK_SEMI_OPEN; } }
        let mut br=self.black_rooks; while br!=0 { let sq=br.trailing_zeros() as usize; br&=br-1; let f=sq%8; let file_pawns = white_pawns_by_file[f]+black_pawns_by_file[f]; if file_pawns==0 { rook_files -= ROOK_OPEN_FILE; } else if white_pawns_by_file[f]==0 { rook_files -= ROOK_SEMI_OPEN; } }
        // Pawn structure loop (similar to evaluate)
        let file_mask = |f:usize| -> u64 {0x0101010101010101u64 << f};
    for f in 0..8 { if white_pawns_by_file[f] > 1 { pawn_structure -= DOUBLED_PAWN as i32 * (white_pawns_by_file[f] as i32 -1); } if black_pawns_by_file[f] > 1 { pawn_structure += DOUBLED_PAWN as i32 * (black_pawns_by_file[f] as i32 -1); } let left= if f>0 {white_pawns_by_file[f-1]} else {0}; let right= if f<7 {white_pawns_by_file[f+1]} else {0}; if white_pawns_by_file[f]>0 && left==0 && right==0 { pawn_structure -= ISOLATED_PAWN; } let leftb= if f>0 {black_pawns_by_file[f-1]} else {0}; let rightb= if f<7 {black_pawns_by_file[f+1]} else {0}; if black_pawns_by_file[f]>0 && leftb==0 && rightb==0 { pawn_structure += ISOLATED_PAWN; }
            let mut wp_file = self.white_pawns & file_mask(f); while wp_file!=0 { let sq=wp_file.trailing_zeros() as usize; wp_file&=wp_file-1; let rank=sq/8; let mut blocked=false; 'outer: for cf in f.saturating_sub(1)..=usize::min(f+1,7){ let mut bp_scan=self.black_pawns & file_mask(cf); while bp_scan!=0 { let bsq=bp_scan.trailing_zeros() as usize; bp_scan&=bp_scan-1; let br=bsq/8; if br>rank { blocked=true; break 'outer; } } } if !blocked { pawn_structure += PASSED_BASE + (rank as i32)*PASSED_PER_RANK; } if rank<7 { let forward_sq = sq + 8; let left_attack = if f>0 && forward_sq+7 < 64 { self.black_pawns & (1u64 << (forward_sq+7)) } else {0}; let right_attack = if f<7 && forward_sq+9 < 64 { self.black_pawns & (1u64 << (forward_sq+9)) } else {0}; if left_attack|right_attack!=0 { pawn_structure -= BACKWARD_PAWN; } } }
            let mut bp_file = self.black_pawns & file_mask(f); while bp_file!=0 { let sq=bp_file.trailing_zeros() as usize; bp_file&=bp_file-1; let rank=sq/8; let mut blocked=false; 'outerb: for cf in f.saturating_sub(1)..=usize::min(f+1,7){ let mut wp_scan=self.white_pawns & file_mask(cf); while wp_scan!=0 { let wsq=wp_scan.trailing_zeros() as usize; wp_scan&=wp_scan-1; let wr=wsq/8; if wr<rank { blocked=true; break 'outerb; } } } if !blocked { pawn_structure -= PASSED_BASE + ((7-rank) as i32)*PASSED_PER_RANK; } if rank>0 { let forward_sq=sq-8; let left_attack = if f>0 && forward_sq >= 9 { self.white_pawns & (1u64 << (forward_sq-9)) } else {0}; let right_attack = if f<7 && forward_sq >=7 { self.white_pawns & (1u64 << (forward_sq-7)) } else {0}; if left_attack|right_attack!=0 { pawn_structure += BACKWARD_PAWN; } } }
        }
        // Hanging
        let mut w_iter = self.white_pieces() & !(1u64<<w_king_sq); while w_iter!=0 { let sq=w_iter.trailing_zeros() as usize; w_iter&=w_iter-1; if (black_attacks & (1u64<<sq))!=0 && (white_attacks & (1u64<<sq))==0 { hanging -= HANGING_PENALTY; } }
        let mut b_iter = self.black_pieces() & !(1u64<<b_king_sq); while b_iter!=0 { let sq=b_iter.trailing_zeros() as usize; b_iter&=b_iter-1; if (white_attacks & (1u64<<sq))!=0 && (black_attacks & (1u64<<sq))==0 { hanging += HANGING_PENALTY; } }
        tempo += if us_white { TEMPO } else { -TEMPO };
    let mut total = material + piece_square + mobility + king_ring_score + bishop_pair_score + rook_files + pawn_structure + hanging + tempo;
        let non_pawn_material_white = (self.white_knights.count_ones()+self.white_bishops.count_ones()) as i32*325 + self.white_rooks.count_ones() as i32*500 + self.white_queens.count_ones() as i32*950;
        let non_pawn_material_black = (self.black_knights.count_ones()+self.black_bishops.count_ones()) as i32*325 + self.black_rooks.count_ones() as i32*500 + self.black_queens.count_ones() as i32*950;
        let total_np = non_pawn_material_white + non_pawn_material_black; let pre_scale = total; let mut scaled = total; let mut scaled_flag=false;
        if total_np < params.endgame_threshold { let scale = 32 + total_np / 80; scaled = scaled * scale / 64; scaled_flag=true; }
    EvalBreakdown { material, piece_square, mobility, king_ring: king_ring_score, bishop_pair: bishop_pair_score, rook_files, pawn_structure, hanging, tempo, endgame_scale_applied: scaled_flag, scaled_total: if self.white_to_move { scaled } else { -scaled }, pre_scale_total: if self.white_to_move { pre_scale } else { -pre_scale } }
    }

    pub fn evaluate_with_breakdown(&self) -> EvalBreakdown { self.evaluate_with_breakdown_params(&EvalParams::default()) }

    pub(crate) fn new() -> Self {
        // Initialize bitboards for starting position
        let white_pawns = 0xFF00u64; // Rank 2
        let black_pawns = 0xFF000000000000u64; // Rank 7
        let white_rooks = 0x81u64; // a1, h1
        let black_rooks = 0x8100000000000000u64; // a8, h8
        let white_knights = 0x42u64; // b1, g1
        let black_knights = 0x4200000000000000u64; // b8, g8
        let white_bishops = 0x24u64; // c1, f1
        let black_bishops = 0x2400000000000000u64; // c8, f8
        let white_queens = 0x08u64; // d1
        let black_queens = 0x0800000000000000u64; // d8  
        let white_king = 0x10u64; // e1
        let black_king = 0x1000000000000000u64; // e8
        
        let mut castling_rights = HashSet::new();
        castling_rights.insert((Color::White, 'K'));
        castling_rights.insert((Color::White, 'Q'));
        castling_rights.insert((Color::Black, 'K'));
        castling_rights.insert((Color::Black, 'Q'));
        
        let mut board = Board { 
            white_pawns, white_knights, white_bishops, white_rooks, white_queens, white_king,
            black_pawns, black_knights, black_bishops, black_rooks, black_queens, black_king,
            white_to_move: true, en_passant_target: None, castling_rights, hash: 0, halfmove_clock: 0, position_history: Vec::new() 
        };
        board.hash = board.calculate_initial_hash();
        board.position_history.push(board.hash);
        board
    }
    
    // Helper methods to query pieces at squares
    pub(crate) fn piece_at(&self, sq: Square) -> Option<(Color, Piece)> {
        let bit = 1u64 << (sq.0 * 8 + sq.1);
        
        if (self.white_pawns & bit) != 0 { return Some((Color::White, Piece::Pawn)); }
        if (self.white_knights & bit) != 0 { return Some((Color::White, Piece::Knight)); }
        if (self.white_bishops & bit) != 0 { return Some((Color::White, Piece::Bishop)); }
        if (self.white_rooks & bit) != 0 { return Some((Color::White, Piece::Rook)); }
        if (self.white_queens & bit) != 0 { return Some((Color::White, Piece::Queen)); }
        if (self.white_king & bit) != 0 { return Some((Color::White, Piece::King)); }
        
        if (self.black_pawns & bit) != 0 { return Some((Color::Black, Piece::Pawn)); }
        if (self.black_knights & bit) != 0 { return Some((Color::Black, Piece::Knight)); }
        if (self.black_bishops & bit) != 0 { return Some((Color::Black, Piece::Bishop)); }
        if (self.black_rooks & bit) != 0 { return Some((Color::Black, Piece::Rook)); }
        if (self.black_queens & bit) != 0 { return Some((Color::Black, Piece::Queen)); }
        if (self.black_king & bit) != 0 { return Some((Color::Black, Piece::King)); }
        
        None
    }
    
    pub(crate) fn all_pieces(&self) -> u64 {
        self.white_pawns | self.white_knights | self.white_bishops | self.white_rooks | self.white_queens | self.white_king |
        self.black_pawns | self.black_knights | self.black_bishops | self.black_rooks | self.black_queens | self.black_king
    }
    
    pub(crate) fn white_pieces(&self) -> u64 {
        self.white_pawns | self.white_knights | self.white_bishops | self.white_rooks | self.white_queens | self.white_king
    }
    
    pub(crate) fn black_pieces(&self) -> u64 {
        self.black_pawns | self.black_knights | self.black_bishops | self.black_rooks | self.black_queens | self.black_king
    }
    
    pub(crate) fn pieces_of_color(&self, color: Color) -> u64 {
        match color {
            Color::White => self.white_pieces(),
            Color::Black => self.black_pieces(),
        }
    }
    
    pub(crate) fn set_piece_at(&mut self, sq: Square, color: Color, piece: Piece) {
        let bit = 1u64 << (sq.0 * 8 + sq.1);
        self.clear_square(sq); // Remove any existing piece
        
        match (color, piece) {
            (Color::White, Piece::Pawn) => self.white_pawns |= bit,
            (Color::White, Piece::Knight) => self.white_knights |= bit,
            (Color::White, Piece::Bishop) => self.white_bishops |= bit,
            (Color::White, Piece::Rook) => self.white_rooks |= bit,
            (Color::White, Piece::Queen) => self.white_queens |= bit,
            (Color::White, Piece::King) => self.white_king |= bit,
            (Color::Black, Piece::Pawn) => self.black_pawns |= bit,
            (Color::Black, Piece::Knight) => self.black_knights |= bit,
            (Color::Black, Piece::Bishop) => self.black_bishops |= bit,
            (Color::Black, Piece::Rook) => self.black_rooks |= bit,
            (Color::Black, Piece::Queen) => self.black_queens |= bit,
            (Color::Black, Piece::King) => self.black_king |= bit,
        }
    }
    
    pub(crate) fn clear_square(&mut self, sq: Square) {
        let bit = !(1u64 << (sq.0 * 8 + sq.1));
        self.white_pawns &= bit;
        self.white_knights &= bit;
        self.white_bishops &= bit;
        self.white_rooks &= bit;
        self.white_queens &= bit;
        self.white_king &= bit;
        self.black_pawns &= bit;
        self.black_knights &= bit;
        self.black_bishops &= bit;
        self.black_rooks &= bit;
        self.black_queens &= bit;
        self.black_king &= bit;
    }

    pub(crate) fn from_fen(fen: &str) -> Self {
        // Initialize empty bitboards
        let mut board = Board {
            white_pawns: 0, white_knights: 0, white_bishops: 0, white_rooks: 0, white_queens: 0, white_king: 0,
            black_pawns: 0, black_knights: 0, black_bishops: 0, black_rooks: 0, black_queens: 0, black_king: 0,
            white_to_move: true, en_passant_target: None, castling_rights: HashSet::new(), hash: 0, halfmove_clock: 0, position_history: Vec::new()
        };
        
        let parts: Vec<&str> = fen.split_whitespace().collect();
        assert!(parts.len() >= 4, "FEN must have at least 4 parts");
        for (rank_idx, rank_str) in parts[0].split('/').enumerate() {
            let mut file = 0;
            for c in rank_str.chars() {
                if c.is_digit(10) { file += c.to_digit(10).unwrap() as usize; }
                else {
                    let (color, piece) = match c {
                        'P' => (Color::White, Piece::Pawn), 'N' => (Color::White, Piece::Knight), 'B' => (Color::White, Piece::Bishop), 'R' => (Color::White, Piece::Rook), 'Q' => (Color::White, Piece::Queen), 'K' => (Color::White, Piece::King),
                        'p' => (Color::Black, Piece::Pawn), 'n' => (Color::Black, Piece::Knight), 'b' => (Color::Black, Piece::Bishop), 'r' => (Color::Black, Piece::Rook), 'q' => (Color::Black, Piece::Queen), 'k' => (Color::Black, Piece::King),
                        _ => panic!("Invalid piece char"), };
                    board.set_piece_at(Square(7 - rank_idx, file), color, piece);
                    file += 1;
                }
            }
        }
        board.white_to_move = match parts[1] { "w" => true, "b" => false, _ => panic!("Invalid color") };
        for c in parts[2].chars() {
            match c { 'K' => { board.castling_rights.insert((Color::White, 'K')); }, 'Q' => { board.castling_rights.insert((Color::White, 'Q')); }, 'k' => { board.castling_rights.insert((Color::Black, 'K')); }, 'q' => { board.castling_rights.insert((Color::Black, 'Q')); }, '-' => {}, _ => panic!("Invalid castle"), }
        }
        board.en_passant_target = if parts[3] != "-" { let chars: Vec<char> = parts[3].chars().collect(); if chars.len() == 2 { Some(Square(rank_to_index(chars[1]), file_to_index(chars[0]))) } else { None } } else { None };
        board.halfmove_clock = parts.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        board.hash = board.calculate_initial_hash();
        board.position_history.push(board.hash);
        board
    }

    // Calculate Zobrist hash from scratch
    fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;
        
        // Hash all pieces from bitboards
        let pieces = [
            (self.white_pawns, Color::White, Piece::Pawn),
            (self.white_knights, Color::White, Piece::Knight),
            (self.white_bishops, Color::White, Piece::Bishop),
            (self.white_rooks, Color::White, Piece::Rook),
            (self.white_queens, Color::White, Piece::Queen),
            (self.white_king, Color::White, Piece::King),
            (self.black_pawns, Color::Black, Piece::Pawn),
            (self.black_knights, Color::Black, Piece::Knight),
            (self.black_bishops, Color::Black, Piece::Bishop),
            (self.black_rooks, Color::Black, Piece::Rook),
            (self.black_queens, Color::Black, Piece::Queen),
            (self.black_king, Color::Black, Piece::King),
        ];
        
        for (bb, color, piece) in pieces {
            let mut pieces_bb = bb;
            while pieces_bb != 0 {
                let sq_idx = pieces_bb.trailing_zeros() as usize;
                pieces_bb &= pieces_bb - 1; // Clear the lowest set bit
                let sq = Square(sq_idx / 8, sq_idx % 8);
                let sq_zobrist_idx = square_to_zobrist_index(sq);
                let p_idx = piece_to_zobrist_index(piece);
                let c_idx = color_to_zobrist_index(color);
                hash ^= ZOBRIST.piece_keys[p_idx][c_idx][sq_zobrist_idx];
            }
        }
        
        if !self.white_to_move { hash ^= ZOBRIST.black_to_move_key; }
        if self.castling_rights.contains(&(Color::White, 'K')) { hash ^= ZOBRIST.castling_keys[0][0]; }
        if self.castling_rights.contains(&(Color::White, 'Q')) { hash ^= ZOBRIST.castling_keys[0][1]; }
        if self.castling_rights.contains(&(Color::Black, 'K')) { hash ^= ZOBRIST.castling_keys[1][0]; }
        if self.castling_rights.contains(&(Color::Black, 'Q')) { hash ^= ZOBRIST.castling_keys[1][1]; }
        if let Some(ep_square) = self.en_passant_target { hash ^= ZOBRIST.en_passant_keys[ep_square.1]; }
        hash
    }

    // --- Make/Unmake Logic ---
    pub(crate) fn make_move(&mut self, m: &Move) -> UnmakeInfo {
        let mut current_hash = self.hash;
        let previous_hash = self.hash;
        let color = self.current_color();
        let previous_en_passant_target = self.en_passant_target;
        let previous_castling_rights = self.castling_rights.clone();
        let previous_halfmove_clock = self.halfmove_clock;
        current_hash ^= ZOBRIST.black_to_move_key;
        if let Some(old_ep) = self.en_passant_target { current_hash ^= ZOBRIST.en_passant_keys[old_ep.1]; }
        let mut captured_piece_info: Option<(Color, Piece)> = None;
        if m.is_en_passant {
            let capture_row = if color == Color::White { m.to.0 - 1 } else { m.to.0 + 1 };
            let capture_sq = Square(capture_row, m.to.1);
            let cap_idx = square_to_zobrist_index(capture_sq);
            captured_piece_info = self.piece_at(capture_sq);
            self.clear_square(capture_sq);
            if let Some((cap_col, cap_piece)) = captured_piece_info { current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)][color_to_zobrist_index(cap_col)][cap_idx]; }
        } else if !m.is_castling {
            captured_piece_info = self.piece_at(m.to);
            if captured_piece_info.is_some() {
                let cap_idx = square_to_zobrist_index(m.to);
                if let Some((cap_col, cap_piece)) = captured_piece_info { current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(cap_piece)][color_to_zobrist_index(cap_col)][cap_idx]; }
            }
        }
        let moving_piece_info = self.piece_at(m.from).expect("make_move 'from' empty");
        let (moving_color, moving_piece) = moving_piece_info;
        let from_sq_idx = square_to_zobrist_index(m.from);
        let to_sq_idx = square_to_zobrist_index(m.to);
        current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(moving_piece)][color_to_zobrist_index(moving_color)][from_sq_idx];
        self.clear_square(m.from);
        if m.is_castling {
            self.set_piece_at(m.to, color, Piece::King);
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::King)][color_to_zobrist_index(color)][to_sq_idx];
            let (rook_from_f, rook_to_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_from_sq = Square(m.to.0, rook_from_f);
            let rook_to_sq = Square(m.to.0, rook_to_f);
            let rook_info = self.piece_at(rook_from_sq).expect("Castling without rook");
            self.clear_square(rook_from_sq);
            self.set_piece_at(rook_to_sq, rook_info.0, rook_info.1);
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)][color_to_zobrist_index(color)][square_to_zobrist_index(rook_from_sq)];
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(Piece::Rook)][color_to_zobrist_index(color)][square_to_zobrist_index(rook_to_sq)];
        } else {
            let piece_to_place = if let Some(promoted_piece) = m.promotion { (color, promoted_piece) } else { moving_piece_info };
            self.set_piece_at(m.to, piece_to_place.0, piece_to_place.1);
            current_hash ^= ZOBRIST.piece_keys[piece_to_zobrist_index(piece_to_place.1)][color_to_zobrist_index(piece_to_place.0)][to_sq_idx];
        }
        self.en_passant_target = None;
        if moving_piece == Piece::Pawn && (m.from.0 as isize - m.to.0 as isize).abs() == 2 {
            let ep_row = (m.from.0 + m.to.0) / 2; let ep_sq = Square(ep_row, m.from.1); self.en_passant_target = Some(ep_sq); current_hash ^= ZOBRIST.en_passant_keys[ep_sq.1];
        }
        if moving_piece == Piece::Pawn || captured_piece_info.is_some() { self.halfmove_clock = 0; } else { self.halfmove_clock = self.halfmove_clock.saturating_add(1); }
        let mut new_castling_rights = self.castling_rights.clone(); let mut castle_hash_diff: u64 = 0;
        if moving_piece == Piece::King {
            if self.castling_rights.contains(&(color, 'K')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0]; new_castling_rights.remove(&(color, 'K')); }
            if self.castling_rights.contains(&(color, 'Q')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1]; new_castling_rights.remove(&(color, 'Q')); }
        } else if moving_piece == Piece::Rook {
            let start_rank = if color == Color::White { 0 } else { 7 };
            if m.from == Square(start_rank, 0) && self.castling_rights.contains(&(color, 'Q')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][1]; new_castling_rights.remove(&(color, 'Q')); }
            else if m.from == Square(start_rank, 7) && self.castling_rights.contains(&(color, 'K')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(color)][0]; new_castling_rights.remove(&(color, 'K')); }
        }
        if let Some((captured_color, captured_piece)) = captured_piece_info { if captured_piece == Piece::Rook { let start_rank = if captured_color == Color::White { 0 } else { 7 }; if m.to == Square(start_rank, 0) && self.castling_rights.contains(&(captured_color, 'Q')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][1]; new_castling_rights.remove(&(captured_color, 'Q')); } else if m.to == Square(start_rank, 7) && self.castling_rights.contains(&(captured_color, 'K')) { castle_hash_diff ^= ZOBRIST.castling_keys[color_to_zobrist_index(captured_color)][0]; new_castling_rights.remove(&(captured_color, 'K')); } } }
        self.castling_rights = new_castling_rights; current_hash ^= castle_hash_diff;
        self.white_to_move = !self.white_to_move;
        self.hash = current_hash; self.position_history.push(self.hash);
        UnmakeInfo { captured_piece_info, previous_en_passant_target, previous_castling_rights, previous_hash, previous_halfmove_clock }
    }

    pub(crate) fn unmake_move(&mut self, m: &Move, info: UnmakeInfo) {
        let _ = self.position_history.pop();
        self.white_to_move = !self.white_to_move;
        self.en_passant_target = info.previous_en_passant_target;
        self.castling_rights = info.previous_castling_rights;
        self.hash = info.previous_hash;
        self.halfmove_clock = info.previous_halfmove_clock;
        let color = self.current_color();
        let piece_that_moved = if m.promotion.is_some() { (color, Piece::Pawn) } else if m.is_castling { (color, Piece::King) } else { self.piece_at(m.to).expect("Unmake move: 'to' square empty?") };
        if m.is_castling {
            self.set_piece_at(m.from, piece_that_moved.0, piece_that_moved.1); self.clear_square(m.to);
            let (rook_orig_f, rook_moved_f) = if m.to.1 == 6 { (7, 5) } else { (0, 3) };
            let rook_info = self.piece_at(Square(m.to.0, rook_moved_f)).expect("Unmake castling: rook missing");
            self.clear_square(Square(m.to.0, rook_moved_f)); self.set_piece_at(Square(m.to.0, rook_orig_f), rook_info.0, rook_info.1);
        } else {
            self.set_piece_at(m.from, piece_that_moved.0, piece_that_moved.1);
            if m.is_en_passant {
                self.clear_square(m.to); let capture_row = if color == Color::White { m.to.0 - 1 } else { m.to.0 + 1 }; 
                if let Some((cap_color, cap_piece)) = info.captured_piece_info { self.set_piece_at(Square(capture_row, m.to.1), cap_color, cap_piece); }
            } else { 
                if let Some((cap_color, cap_piece)) = info.captured_piece_info { self.set_piece_at(m.to, cap_color, cap_piece); } else { self.clear_square(m.to); }
            }
        }
    }

    pub(crate) fn make_null_move(&mut self) -> (Option<Square>, u64, u32) {
        let prev_ep = self.en_passant_target; let prev_hash = self.hash; let prev_halfmove = self.halfmove_clock;
        if let Some(ep) = self.en_passant_target { self.hash ^= ZOBRIST.en_passant_keys[ep.1]; }
        self.white_to_move = !self.white_to_move; self.hash ^= ZOBRIST.black_to_move_key; self.en_passant_target = None; self.halfmove_clock = self.halfmove_clock.saturating_add(1); self.position_history.push(self.hash);
        (prev_ep, prev_hash, prev_halfmove)
    }

    pub(crate) fn unmake_null_move(&mut self, prev_ep: Option<Square>, prev_hash: u64, prev_halfmove: u32) {
        let _ = self.position_history.pop(); self.hash = prev_hash; self.white_to_move = !self.white_to_move; self.en_passant_target = prev_ep; self.halfmove_clock = prev_halfmove;
    }

    fn generate_pseudo_moves(&self) -> Vec<Move> {
        self.generate_pseudo_moves_bb()
    }

    fn generate_pseudo_moves_bb(&self) -> Vec<Move> {
        use crate::bitboards as bb;
        let mut moves = Vec::new();
        let color = self.current_color();
        let us = color;
        let _them = self.opponent_color(color);

        // Use existing bitboards
        let occ = self.all_pieces();
        let occ_us = self.pieces_of_color(us);
        let occ_them = self.pieces_of_color(self.opponent_color(us));
        let (pawns, knights, bishops, rooks, queens, king) = match us {
            Color::White => (self.white_pawns, self.white_knights, self.white_bishops, self.white_rooks, self.white_queens, self.white_king),
            Color::Black => (self.black_pawns, self.black_knights, self.black_bishops, self.black_rooks, self.black_queens, self.black_king),
        };

        // Helper to add a quiet or capture move from bit indices
        let mut push_move = |from_idx: usize, to_idx: usize, promo: Option<Piece>, is_ep: bool| {
            let from = Square(from_idx/8, from_idx%8);
            let to = Square(to_idx/8, to_idx%8);
            let captured_piece = if is_ep { Some(Piece::Pawn) } else { self.piece_at(to).map(|(_,p)| p) };
            moves.push(Move{ from, to, is_castling:false, is_en_passant:is_ep, promotion:promo, captured_piece });
        };

        // Knights first - prioritize piece development over pawn pushes
        let mut n = knights; while n!=0 { let from = n.trailing_zeros() as usize; n &= n-1; let from_sq = Square(from/8, from%8); let mut targets = bb::knight_attacks_from(from_sq) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); } }

        // Bishops second  
        let mut b = bishops; while b!=0 { let from = b.trailing_zeros() as usize; b &= b-1; let from_sq = Square(from/8, from%8); let mut targets = bb::bishop_attacks_from(from_sq, occ) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); } }

        // Pawn captures first (higher priority than quiet pawn moves)
        if pawns != 0 {
            if us == Color::White {
                // Captures first
                let left_capt = (pawns << 7) & !bb::FILE_H & occ_them;
                let right_capt = (pawns << 9) & !bb::FILE_A & occ_them;
                let mut lc = left_capt; while lc!=0 { let to = lc.trailing_zeros() as usize; lc &= lc-1; let from = to-7; if to>=56 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight]{ push_move(from,to,Some(promo),false);} } else { push_move(from,to,None,false);} }
                let mut rc = right_capt; while rc!=0 { let to = rc.trailing_zeros() as usize; rc &= rc-1; let from = to-9; if to>=56 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight]{ push_move(from,to,Some(promo),false);} } else { push_move(from,to,None,false);} }
                // En passant
                if let Some(ep) = self.en_passant_target { let ep_bb = bb::sq_to_bb(ep); let left_ep = (pawns << 7) & !bb::FILE_H & ep_bb; if left_ep!=0 { let to = ep.0*8+ep.1; let from = to-7; push_move(from,to,None,true); } let right_ep = (pawns << 9) & !bb::FILE_A & ep_bb; if right_ep!=0 { let to = ep.0*8+ep.1; let from = to-9; push_move(from,to,None,true); } }
                // Pushes second
                let empty = !occ;
                let one = (pawns << 8) & empty;
                // Promotions on rank 8
                let promos = one & bb::RANK8;
                let quiets = one & !bb::RANK8;
                let mut q = quiets; while q!=0 { let to = q.trailing_zeros() as usize; q &= q-1; let from = to-8; push_move(from, to, None, false); }
                let mut pr = promos; while pr!=0 { let to = pr.trailing_zeros() as usize; pr &= pr-1; let from = to-8; for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight] { push_move(from,to,Some(promo),false); } }
                // Double pushes from rank2
                let two = ((one & bb::rank3_mask()) << 8) & empty; // rank3_mask computed below via helper
                let mut t = two; while t!=0 { let to = t.trailing_zeros() as usize; t &= t-1; let from = to-16; push_move(from, to, None, false); }
            } else {
                // Black - captures first
                let left_capt = (pawns >> 9) & !bb::FILE_H & occ_them;
                let right_capt = (pawns >> 7) & !bb::FILE_A & occ_them;
                let mut lc = left_capt; while lc!=0 { let to = lc.trailing_zeros() as usize; lc &= lc-1; let from = to+9; if to<8 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight]{ push_move(from,to,Some(promo),false);} } else { push_move(from,to,None,false);} }
                let mut rc = right_capt; while rc!=0 { let to = rc.trailing_zeros() as usize; rc &= rc-1; let from = to+7; if to<8 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight]{ push_move(from,to,Some(promo),false);} } else { push_move(from,to,None,false);} }
                // EP
                if let Some(ep) = self.en_passant_target { let ep_bb = bb::sq_to_bb(ep); let left_ep = (pawns >> 9) & !bb::FILE_H & ep_bb; if left_ep!=0 { let to = ep.0*8+ep.1; let from = to+9; push_move(from,to,None,true); } let right_ep = (pawns >> 7) & !bb::FILE_A & ep_bb; if right_ep!=0 { let to = ep.0*8+ep.1; let from = to+7; push_move(from,to,None,true); } }
                // Pushes second
                let empty = !occ;
                let one = (pawns >> 8) & empty;
                let promos = one & bb::RANK1;
                let quiets = one & !bb::RANK1;
                let mut q = quiets; while q!=0 { let to = q.trailing_zeros() as usize; q &= q-1; let from = to+8; push_move(from, to, None, false); }
                let mut pr = promos; while pr!=0 { let to = pr.trailing_zeros() as usize; pr &= pr-1; let from = to+8; for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight] { push_move(from,to,Some(promo),false); } }
                let two = ((one & bb::rank6_mask()) >> 8) & empty;
                let mut t = two; while t!=0 { let to = t.trailing_zeros() as usize; t &= t-1; let from = to+16; push_move(from, to, None, false); }
            }
        }

        // Rooks
        let mut r = rooks; while r!=0 { let from = r.trailing_zeros() as usize; r &= r-1; let from_sq = Square(from/8, from%8); let mut targets = bb::rook_attacks_from(from_sq, occ) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); } }

        // Queens
        let mut qn = queens; while qn!=0 { let from = qn.trailing_zeros() as usize; qn &= qn-1; let from_sq = Square(from/8, from%8); let mut targets = (bb::rook_attacks_from(from_sq, occ) | bb::bishop_attacks_from(from_sq, occ)) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); } }

        // King (including castling squares without safety check here; legality filter later will exclude illegal castling through check)
        if king != 0 { let from = king.trailing_zeros() as usize; let from_sq = Square(from/8, from%8); let mut targets = bb::king_attacks_from(from_sq) & !occ_us; while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; push_move(from,to,None,false); }
            // Castling pseudo-moves
            let back_rank = if us==Color::White {0} else {7};
            if from_sq == Square(back_rank,4) {
                // King side
                if self.castling_rights.contains(&(us,'K')) && self.piece_at(Square(back_rank,5)).is_none() && self.piece_at(Square(back_rank,6)).is_none() && self.piece_at(Square(back_rank,7))==Some((us,Piece::Rook)) {
                    moves.push(Move{ from:from_sq, to:Square(back_rank,6), is_castling:true, is_en_passant:false, promotion:None, captured_piece:None});
                }
                // Queen side
                if self.castling_rights.contains(&(us,'Q')) && self.piece_at(Square(back_rank,1)).is_none() && self.piece_at(Square(back_rank,2)).is_none() && self.piece_at(Square(back_rank,3)).is_none() && self.piece_at(Square(back_rank,0))==Some((us,Piece::Rook)) {
                    moves.push(Move{ from:from_sq, to:Square(back_rank,2), is_castling:true, is_en_passant:false, promotion:None, captured_piece:None});
                }
            }
        }

        moves
    }

    pub(crate) fn generate_piece_moves(&self, from: Square, piece: Piece) -> Vec<Move> {
        use crate::bitboards as bb;
        // Build occupancy masks
        let mut occ: u64 = 0; let mut occ_us: u64 = 0;
        let us = self.current_color();
        let occ = self.all_pieces();
        let occ_us = self.pieces_of_color(us);

        match piece {
            Piece::Pawn => self.generate_pawn_moves(from),
            Piece::Knight => {
                let mut moves = Vec::new();
                let mut targets = bb::knight_attacks_from(from) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                moves
            }
            Piece::Bishop => {
                let mut moves = Vec::new();
                let mut targets = bb::bishop_attacks_from(from, occ) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                moves
            }
            Piece::Rook => {
                let mut moves = Vec::new();
                let mut targets = bb::rook_attacks_from(from, occ) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                moves
            }
            Piece::Queen => {
                let mut moves = Vec::new();
                let mut targets = (bb::rook_attacks_from(from, occ) | bb::bishop_attacks_from(from, occ)) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                moves
            }
            Piece::King => {
                // Single king moves via bitboards + castling pseudo as before
                let mut moves = Vec::new();
                let mut targets = bb::king_attacks_from(from) & !occ_us;
                while targets!=0 { let to = targets.trailing_zeros() as usize; targets &= targets-1; let to_sq = Square(to/8, to%8); let captured_piece = self.piece_at(to_sq).map(|(_,p)| p); moves.push(Move{ from, to:to_sq, is_castling:false, is_en_passant:false, promotion:None, captured_piece}); }
                // Castling pseudo-moves
                let color = self.current_color();
                let back_rank = if color == Color::White { 0 } else { 7 };
                if from == Square(back_rank, 4) {
                    if self.castling_rights.contains(&(color, 'K')) && self.piece_at(Square(back_rank,5)).is_none() && self.piece_at(Square(back_rank,6)).is_none() && self.piece_at(Square(back_rank,7)) == Some((color, Piece::Rook)) {
                        moves.push(Move{ from, to:Square(back_rank,6), is_castling:true, is_en_passant:false, promotion:None, captured_piece:None});
                    }
                    if self.castling_rights.contains(&(color, 'Q')) && self.piece_at(Square(back_rank,1)).is_none() && self.piece_at(Square(back_rank,2)).is_none() && self.piece_at(Square(back_rank,3)).is_none() && self.piece_at(Square(back_rank,0)) == Some((color, Piece::Rook)) {
                        moves.push(Move{ from, to:Square(back_rank,2), is_castling:true, is_en_passant:false, promotion:None, captured_piece:None});
                    }
                }
                moves
            }
        }
    }

    fn create_move(&self, from: Square, to: Square, promotion: Option<Piece>, is_castling: bool, is_en_passant: bool) -> Move {
        let captured_piece = if is_en_passant { Some(Piece::Pawn) } else if !is_castling { self.piece_at(to).map(|(_, p)| p) } else { None };
        Move { from, to, promotion, is_castling, is_en_passant, captured_piece }
    }

    fn generate_pawn_moves(&self, from: Square) -> Vec<Move> {
        let color = if self.white_to_move { Color::White } else { Color::Black };
        let mut moves = Vec::new();
        let dir: isize = if color == Color::White { 1 } else { -1 };
        let start_rank = if color == Color::White { 1 } else { 6 };
        let promotion_rank = if color == Color::White { 7 } else { 0 };
        let r = from.0 as isize; let f = from.1 as isize;
        let forward_r = r + dir;
        if forward_r >= 0 && forward_r < 8 { let forward_sq = Square(forward_r as usize, f as usize); if self.piece_at(forward_sq).is_none() { if forward_sq.0 == promotion_rank { for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] { moves.push(self.create_move(from, forward_sq, Some(promo), false, false)); } } else { moves.push(self.create_move(from, forward_sq, None, false, false)); if r == start_rank as isize { let double_forward_r = r + 2 * dir; let double_forward_sq = Square(double_forward_r as usize, f as usize); if self.piece_at(double_forward_sq).is_none() { moves.push(self.create_move(from, double_forward_sq, None, false, false)); } } } } }
        if forward_r >= 0 && forward_r < 8 { for df in [-1, 1] { let capture_f = f + df; if capture_f >= 0 && capture_f < 8 { let target_sq = Square(forward_r as usize, capture_f as usize); if let Some((target_color, _)) = self.piece_at(target_sq) { if target_color != color { if target_sq.0 == promotion_rank { for promo in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] { moves.push(self.create_move(from, target_sq, Some(promo), false, false)); } } else { moves.push(self.create_move(from, target_sq, None, false, false)); } } } else if Some(target_sq) == self.en_passant_target { moves.push(self.create_move(from, target_sq, None, false, true)); } } } }
        moves
    }

    pub(crate) fn generate_pawn_tactical_moves(&self, from: Square, out: &mut Vec<Move>) {
        use crate::bitboards as bb;
        let color = if self.white_to_move { Color::White } else { Color::Black };
        // Build occupancy of opponents to detect captures
        let occ_them = self.pieces_of_color(self.opponent_color(color));

        let from_idx = from.0*8 + from.1;
        if color == Color::White {
            // White captures: left (<<7, not on H-file), right (<<9, not on A-file)
            let left = ((1u64 << from_idx) << 7) & !bb::FILE_H & occ_them;
            let right = ((1u64 << from_idx) << 9) & !bb::FILE_A & occ_them;
            let mut caps = left | right;
            while caps != 0 { let to = caps.trailing_zeros() as usize; caps &= caps-1; let to_sq = Square(to/8,to%8); if to_sq.0 == 7 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight] { out.push(self.create_move(from, to_sq, Some(promo), false, false)); } } else { out.push(self.create_move(from, to_sq, None, false, false)); } }
            // En passant
            if let Some(ep) = self.en_passant_target { let ep_bb = bb::sq_to_bb(ep); let left_ep = ((1u64<<from_idx) << 7) & !bb::FILE_H & ep_bb; if left_ep!=0 { out.push(self.create_move(from, ep, None, false, true)); } let right_ep = ((1u64<<from_idx) << 9) & !bb::FILE_A & ep_bb; if right_ep!=0 { out.push(self.create_move(from, ep, None, false, true)); } }
        } else {
            // Black captures
            let left = ((1u64 << from_idx) >> 9) & !bb::FILE_H & occ_them;
            let right = ((1u64 << from_idx) >> 7) & !bb::FILE_A & occ_them;
            let mut caps = left | right;
            while caps != 0 { let to = caps.trailing_zeros() as usize; caps &= caps-1; let to_sq = Square(to/8,to%8); if to_sq.0 == 0 { for promo in [Piece::Queen,Piece::Rook,Piece::Bishop,Piece::Knight] { out.push(self.create_move(from, to_sq, Some(promo), false, false)); } } else { out.push(self.create_move(from, to_sq, None, false, false)); } }
            if let Some(ep) = self.en_passant_target { let ep_bb = bb::sq_to_bb(ep); let left_ep = ((1u64<<from_idx) >> 9) & !bb::FILE_H & ep_bb; if left_ep!=0 { out.push(self.create_move(from, ep, None, false, true)); } let right_ep = ((1u64<<from_idx) >> 7) & !bb::FILE_A & ep_bb; if right_ep!=0 { out.push(self.create_move(from, ep, None, false, true)); } }
        }
    }

    fn generate_knight_moves(&self, from: Square) -> Vec<Move> {
        let mut moves = Vec::new();
        let deltas = [(2, 1),(1, 2),(-1, 2),(-2, 1),(-2, -1),(-1, -2),(1, -2),(2, -1)];
        let (rank, file) = (from.0 as isize, from.1 as isize);
        let color = self.current_color();
        for (dr, df) in deltas { let (nr, nf) = (rank + dr, file + df); if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 { let to_sq = Square(nr as usize, nf as usize); if let Some((c, _)) = self.piece_at(to_sq) { if c != color { moves.push(self.create_move(from, to_sq, None, false, false)); } } else { moves.push(self.create_move(from, to_sq, None, false, false)); } } }
        moves
    }

    fn generate_sliding_moves(&self, from: Square, directions: &[(isize, isize)]) -> Vec<Move> {
        let mut moves = Vec::new();
        let (rank, file) = (from.0 as isize, from.1 as isize);
        let color = self.current_color();
        for &(dr, df) in directions { let mut r = rank + dr; let mut f = file + df; while r >= 0 && r < 8 && f >= 0 && f < 8 { let to_sq = Square(r as usize, f as usize); if let Some((c, _)) = self.piece_at(to_sq) { if c != color { moves.push(self.create_move(from, to_sq, None, false, false)); } break; } else { moves.push(self.create_move(from, to_sq, None, false, false)); } r += dr; f += df; } }
        moves
    }

    fn generate_king_moves(&self, from: Square) -> Vec<Move> {
        let mut moves = Vec::new();
        let deltas = [(1, 0),(-1, 0),(0, 1),(0, -1),(1, 1),(1, -1),(-1, 1),(-1, -1)];
        let (rank, file) = (from.0, from.1);
        let color = self.current_color();
        let back_rank = if color == Color::White { 0 } else { 7 };
        for (dr, df) in deltas { let (nr, nf) = (rank as isize + dr, file as isize + df); if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 { let to_sq = Square(nr as usize, nf as usize); if let Some((c, _)) = self.piece_at(to_sq) { if c != color { moves.push(self.create_move(from, to_sq, None, false, false)); } } else { moves.push(self.create_move(from, to_sq, None, false, false)); } } }
        if from == Square(back_rank, 4) {
            if self.castling_rights.contains(&(color, 'K')) && self.piece_at(Square(back_rank, 5)).is_none() && self.piece_at(Square(back_rank, 6)).is_none() && self.piece_at(Square(back_rank, 7)) == Some((color, Piece::Rook)) {
                let to_sq = Square(back_rank, 6); moves.push(self.create_move(from, to_sq, None, true, false));
            }
            if self.castling_rights.contains(&(color, 'Q')) && self.piece_at(Square(back_rank, 1)).is_none() && self.piece_at(Square(back_rank, 2)).is_none() && self.piece_at(Square(back_rank, 3)).is_none() && self.piece_at(Square(back_rank, 0)) == Some((color, Piece::Rook)) {
                let to_sq = Square(back_rank, 2); moves.push(self.create_move(from, to_sq, None, true, false));
            }
        }
        moves
    }

    fn find_king(&self, color: Color) -> Option<Square> {
        let king_bb = if color == Color::White { self.white_king } else { self.black_king };
        if king_bb != 0 {
            let sq_idx = king_bb.trailing_zeros() as usize;
            Some(Square(sq_idx / 8, sq_idx % 8))
        } else {
            None
        }
    }

    pub(crate) fn is_square_attacked(&self, square: Square, attacker_color: Color) -> bool {
        crate::bitboards::is_square_attacked_bb(self, square, attacker_color)
    }

    pub(crate) fn is_in_check(&self, color: Color) -> bool { if let Some(king_sq) = self.find_king(color) { self.is_square_attacked(king_sq, self.opponent_color(color)) } else { false } }
    pub(crate) fn is_fifty_move_draw(&self) -> bool { self.halfmove_clock >= 100 }
    pub(crate) fn is_threefold_repetition(&self) -> bool { let current = self.hash; let mut count = 0; for &h in &self.position_history { if h == current { count += 1; } } count >= 3 }

    pub(crate) fn generate_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color(); let opponent_color = self.opponent_color(current_color); let pseudo_moves = self.generate_pseudo_moves(); let mut legal_moves = Vec::new();
        for m in pseudo_moves { if m.is_castling { let king_start_sq = m.from; let king_mid_sq = Square(m.from.0, (m.from.1 + m.to.1) / 2); let king_end_sq = m.to; if self.is_square_attacked(king_start_sq, opponent_color) || self.is_square_attacked(king_mid_sq, opponent_color) || self.is_square_attacked(king_end_sq, opponent_color) { continue; } }
            let info = self.make_move(&m); if !self.is_in_check(current_color) { legal_moves.push(m.clone()); } self.unmake_move(&m, info);
        }
        legal_moves
    }

    pub(crate) fn generate_tactical_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color(); let opponent_color = self.opponent_color(current_color); let pseudo_moves = self.generate_pseudo_moves(); let mut tactical_moves = Vec::new();
        for m in pseudo_moves {
            // Include captures, promotions, and checking moves
            let is_capture = m.captured_piece.is_some();
            let is_promotion = m.promotion.is_some();
            let is_check = {
                let info = self.make_move(&m);
                let gives_check = self.is_in_check(opponent_color);
                self.unmake_move(&m, info);
                gives_check
            };
            
            if is_capture || is_promotion || is_check {
                let info = self.make_move(&m);
                if !self.is_in_check(current_color) { tactical_moves.push(m.clone()); }
                self.unmake_move(&m, info);
            }
        }
        tactical_moves
    }

    pub(crate) fn is_checkmate(&mut self) -> bool { let color = self.current_color(); self.is_in_check(color) && self.generate_moves().is_empty() }
    pub(crate) fn is_stalemate(&mut self) -> bool { let color = self.current_color(); !self.is_in_check(color) && self.generate_moves().is_empty() }

    pub(crate) fn extract_pv(&self, tt: &TranspositionTable, max_depth: usize) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut current_hash = self.hash;
        let mut visited_positions = std::collections::HashSet::new();
        
        for _depth in 0..max_depth {
            if visited_positions.contains(&current_hash) {
                break; // Avoid cycles
            }
            visited_positions.insert(current_hash);
            
            if let Some(entry) = tt.probe(current_hash) {
                if let Some(mv) = &entry.best_move {
                    pv.push(*mv);
                    // Update hash for next iteration (simplified)
                    current_hash = current_hash.wrapping_add(mv.from.0 as u64 + mv.to.0 as u64);
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        
        pv
    }

    pub(crate) fn evaluate(&self) -> i32 {
        self.evaluate_with_breakdown().scaled_total
    }

    // --- SEE helpers and SEE ---
    #[allow(dead_code)]
    fn attackers_to_square(&self, target: Square, color: Color, occ: &[[Option<(Color, Piece)>; 8]; 8]) -> Vec<(Square, Piece)> {
        let mut attackers = Vec::new(); let (tr, tf) = (target.0 as isize, target.1 as isize);
        let pawn_dir: isize = if color == Color::White { -1 } else { 1 };
        for df in [-1, 1] { let r = tr + pawn_dir; let f = tf + df; if r>=0 && r<8 && f>=0 && f<8 { if occ[r as usize][f as usize] == Some((color, Piece::Pawn)) { attackers.push((Square(r as usize, f as usize), Piece::Pawn)); } } }
        let knight_deltas = [(2,1),(1,2),(-1,2),(-2,1),(-2,-1),(-1,-2),(1,-2),(2,-1)];
        for (dr, df) in knight_deltas { let r = tr+dr; let f = tf+df; if r>=0 && r<8 && f>=0 && f<8 { if occ[r as usize][f as usize] == Some((color, Piece::Knight)) { attackers.push((Square(r as usize, f as usize), Piece::Knight)); } } }
        let king_deltas = [(1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)];
        for (dr, df) in king_deltas { let r = tr+dr; let f = tf+df; if r>=0 && r<8 && f>=0 && f<8 { if occ[r as usize][f as usize] == Some((color, Piece::King)) { attackers.push((Square(r as usize, f as usize), Piece::King)); } } }
        let rook_dirs = [(1,0),(-1,0),(0,1),(0,-1)];
        for (dr, df) in rook_dirs { let mut r = tr+dr; let mut f = tf+df; while r>=0 && r<8 && f>=0 && f<8 { if let Some((c,p)) = occ[r as usize][f as usize] { if c==color { if p==Piece::Rook || p==Piece::Queen { attackers.push((Square(r as usize, f as usize), p)); } } break; } r+=dr; f+=df; } }
        let bishop_dirs = [(1,1),(1,-1),(-1,1),(-1,-1)];
        for (dr, df) in bishop_dirs { let mut r = tr+dr; let mut f = tf+df; while r>=0 && r<8 && f>=0 && f<8 { if let Some((c,p)) = occ[r as usize][f as usize] { if c==color { if p==Piece::Bishop || p==Piece::Queen { attackers.push((Square(r as usize, f as usize), p)); } } break; } r+=dr; f+=df; } }
        attackers
    }

    pub(crate) fn see(&self, m: &Move) -> i32 {
        if !(m.captured_piece.is_some() || m.is_en_passant) { return 0; }
        
        // Create a temporary board copy for SEE calculation
        let mut temp_board = Board {
            white_pawns: self.white_pawns,
            white_knights: self.white_knights,
            white_bishops: self.white_bishops,
            white_rooks: self.white_rooks,
            white_queens: self.white_queens,
            white_king: self.white_king,
            black_pawns: self.black_pawns,
            black_knights: self.black_knights,
            black_bishops: self.black_bishops,
            black_rooks: self.black_rooks,
            black_queens: self.black_queens,
            black_king: self.black_king,
            white_to_move: self.white_to_move,
            en_passant_target: self.en_passant_target,
            castling_rights: self.castling_rights.clone(),
            hash: self.hash,
            halfmove_clock: self.halfmove_clock,
            position_history: self.position_history.clone(),
        };
        
        let (attacker_color, mut moving_piece) = self.piece_at(m.from).unwrap();
        if let Some(promo) = m.promotion { moving_piece = promo; }
        let target = m.to;
        let captured_value: i32 = if m.is_en_passant { 
            let cap_row = if attacker_color == Color::White { m.to.0 - 1 } else { m.to.0 + 1 };
            temp_board.clear_square(Square(cap_row, m.to.1));
            piece_value(Piece::Pawn) 
        } else { 
            m.captured_piece.map(piece_value).unwrap_or(0) 
        };
        temp_board.clear_square(m.from);
        temp_board.set_piece_at(target, attacker_color, moving_piece);
        let mut gains: [i32; 32] = [0; 32]; let mut d = 0usize; gains[d] = captured_value; let mut stm = self.opponent_color(attacker_color);
        let select_lva = |list: &[(Square, Piece)]| -> Option<(Square, Piece)> {
            list
                .iter()
                .min_by_key(|(_, p)| piece_value(*p))
                .map(|(sq, p)| (*sq, *p))
        };
        // Simplified SEE - just return the captured piece value for now
        // TODO: Implement full SEE with bitboard-based attacker detection
        captured_value
    }

    pub(crate) fn perft(&mut self, depth: usize) -> u64 {
        if depth == 0 { return 1; }
        let moves = self.generate_moves(); if depth == 1 { return moves.len() as u64; }
        let mut nodes = 0; for m in moves { let info = self.make_move(&m); nodes += self.perft(depth - 1); self.unmake_move(&m, info); }
        nodes
    }

    pub(crate) fn current_color(&self) -> Color { if self.white_to_move { Color::White } else { Color::Black } }
    pub(crate) fn opponent_color(&self, color: Color) -> Color { match color { Color::White => Color::Black, Color::Black => Color::White } }
    
    // Methods needed by publius search
    pub(crate) fn get_opposite_color(&self, color: Color) -> Color {
        self.opponent_color(color)
    }
    
    pub(crate) fn is_draw(&self) -> bool {
        self.is_fifty_move_draw() || self.is_threefold_repetition()
    }
    


    #[allow(dead_code)]
    pub(crate) fn print(&self) {
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() { print!("{} |", rank + 1); for file in 0..8 { let piece_char = match self.piece_at(Square(rank, file)) { Some((Color::White, Piece::Pawn)) => 'P', Some((Color::White, Piece::Knight)) => 'N', Some((Color::White, Piece::Bishop)) => 'B', Some((Color::White, Piece::Rook)) => 'R', Some((Color::White, Piece::Queen)) => 'Q', Some((Color::White, Piece::King)) => 'K', Some((Color::Black, Piece::Pawn)) => 'p', Some((Color::Black, Piece::Knight)) => 'n', Some((Color::Black, Piece::Bishop)) => 'b', Some((Color::Black, Piece::Rook)) => 'r', Some((Color::Black, Piece::Queen)) => 'q', Some((Color::Black, Piece::King)) => 'k', None => ' ', }; print!(" {} |", piece_char); } println!("\n  +---+---+---+---+---+---+---+---+"); }
        println!("    a   b   c   d   e   f   g   h");
        println!("Turn: {}", if self.white_to_move { "White" } else { "Black" });
        if let Some(ep_target) = self.en_passant_target { println!("EP Target: {}", format_square(ep_target)); }
        println!("Castling: {:?}", self.castling_rights);
        println!("------------------------------------");
    }
}

// --- Local helpers for FEN parsing ---
fn file_to_index(file: char) -> usize { file as usize - ('a' as usize) }
fn rank_to_index(rank: char) -> usize { (rank as usize) - ('0' as usize) - 1 }
