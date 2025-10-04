use crate::Board;
use crate::board::Color;
use crate::bitboards as bbm;
use std::io::Write;

// Feature extraction for tuning: exports evaluation component breakdown plus granular
// mobility and pawn-structure counts, king ring attack counts, and optional Stockfish labels.
#[derive(Debug)]
pub struct ExportedFeatures {
    pub fen: String,
    pub material: i32,
    pub piece_square: i32,
    pub mobility: i32,
    pub king_ring: i32,
    pub bishop_pair: i32,
    pub rook_files: i32,
    pub pawn_structure: i32,
    pub hanging: i32,
    pub tempo: i32,
    pub scaled_eval: i32,
    pub pre_scale_eval: i32,
    // Granular mobility counts (targets excluding own pieces)
    pub mob_kn_w: i32, pub mob_kn_b: i32,
    pub mob_bi_w: i32, pub mob_bi_b: i32,
    pub mob_r_w: i32,  pub mob_r_b: i32,
    pub mob_q_w: i32,  pub mob_q_b: i32,
    // Pawn type counts
    pub isolated_w: i32, pub isolated_b: i32,
    pub doubled_w: i32,  pub doubled_b: i32,
    pub backward_w: i32, pub backward_b: i32,
    pub passed_w: i32,   pub passed_b: i32,
    // King ring attack raw counts
    pub kingring_attacks_w: i32, // white attacks on black king ring
    pub kingring_attacks_b: i32, // black attacks on white king ring
    // Optional Stockfish labels (centipawns from side-to-move POV, mate score plies)
    pub stockfish_cp: Option<i32>,
    pub stockfish_mate: Option<i32>,
    // Optional game result label (+1 white win, 0 draw, -1 black win)
    pub game_result: Option<i8>,
}

impl ExportedFeatures {
    pub fn csv_header() -> &'static str {
        "fen,material,piece_square,mobility,king_ring,bishop_pair,rook_files,pawn_structure,hanging,tempo,scaled_eval,pre_scale_eval,\
mob_kn_w,mob_kn_b,mob_bi_w,mob_bi_b,mob_r_w,mob_r_b,mob_q_w,mob_q_b,\
isolated_w,isolated_b,doubled_w,doubled_b,backward_w,backward_b,passed_w,passed_b,\
kingring_attacks_w,kingring_attacks_b,stockfish_cp,stockfish_mate,game_result"
    }
    pub fn to_csv_line(&self) -> String {
        let fmt_opt = |o: Option<i32>| o.map(|v| v.to_string()).unwrap_or_default();
        let fmt_opt_i8 = |o: Option<i8>| o.map(|v| v.to_string()).unwrap_or_default();
    format!("{},{},{},{},{},{},{},{},{},{},{},{},\
{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.fen,
            self.material,self.piece_square,self.mobility,self.king_ring,self.bishop_pair,self.rook_files,self.pawn_structure,self.hanging,self.tempo,self.scaled_eval,self.pre_scale_eval,
            self.mob_kn_w,self.mob_kn_b,self.mob_bi_w,self.mob_bi_b,self.mob_r_w,self.mob_r_b,self.mob_q_w,self.mob_q_b,
            self.isolated_w,self.isolated_b,self.doubled_w,self.doubled_b,self.backward_w,self.backward_b,self.passed_w,self.passed_b,
            self.kingring_attacks_w,self.kingring_attacks_b,
            fmt_opt(self.stockfish_cp),fmt_opt(self.stockfish_mate),fmt_opt_i8(self.game_result)
        )
    }
}

pub fn board_to_fen(board: &Board) -> String {
    // Piece placement
    let mut rows = Vec::new();
    for r in (0..8).rev() { // FEN ranks 8->1
        let mut empty = 0; let mut row = String::new();
        for f in 0..8 { let sq_index = r*8+f; let bb = 1u64 << sq_index; let mut written=false;
            let push_piece = |row: &mut String, ch: char, empty: &mut i32| { if *empty>0 { row.push_str(&empty.to_string()); *empty=0; } row.push(ch); };
            if board.white_pawns & bb !=0 { push_piece(&mut row,'P', &mut empty); written=true; }
            else if board.white_knights & bb !=0 { push_piece(&mut row,'N', &mut empty); written=true; }
            else if board.white_bishops & bb !=0 { push_piece(&mut row,'B', &mut empty); written=true; }
            else if board.white_rooks & bb !=0 { push_piece(&mut row,'R', &mut empty); written=true; }
            else if board.white_queens & bb !=0 { push_piece(&mut row,'Q', &mut empty); written=true; }
            else if board.white_king & bb !=0 { push_piece(&mut row,'K', &mut empty); written=true; }
            else if board.black_pawns & bb !=0 { push_piece(&mut row,'p', &mut empty); written=true; }
            else if board.black_knights & bb !=0 { push_piece(&mut row,'n', &mut empty); written=true; }
            else if board.black_bishops & bb !=0 { push_piece(&mut row,'b', &mut empty); written=true; }
            else if board.black_rooks & bb !=0 { push_piece(&mut row,'r', &mut empty); written=true; }
            else if board.black_queens & bb !=0 { push_piece(&mut row,'q', &mut empty); written=true; }
            else if board.black_king & bb !=0 { push_piece(&mut row,'k', &mut empty); written=true; }
            if !written { empty +=1; }
        }
        if empty>0 { row.push_str(&empty.to_string()); }
        rows.push(row);
    }
    let placement = rows.join("/");
    let stm = if board.white_to_move { "w" } else { "b" };
    // Castling rights
    let mut cr = String::new();
    if board.castling_rights.contains(&(Color::White,'K')) { cr.push('K'); }
    if board.castling_rights.contains(&(Color::White,'Q')) { cr.push('Q'); }
    if board.castling_rights.contains(&(Color::Black,'K')) { cr.push('k'); }
    if board.castling_rights.contains(&(Color::Black,'Q')) { cr.push('q'); }
    if cr.is_empty() { cr.push('-'); }
    // En passant
    let ep = if let Some(sq) = board.en_passant_target { let file = (b'a' + sq.1 as u8) as char; let rank = (b'1' + sq.0 as u8) as char; format!("{}{}", file, rank) } else { "-".to_string() };
    // Halfmove / fullmove (fullmove not tracked; approximate 1)
    format!("{} {} {} {} {} {}", placement, stm, cr, ep, board.halfmove_clock, 1)
}

fn granular_counts(board: &Board) -> (i32,i32,i32,i32,i32,i32,i32,i32, // mobility piece types
                                      i32,i32,i32,i32,i32,i32,i32,i32, // pawn type counts
                                      i32,i32) { // king ring attack counts
    let occ_w = board.white_pieces();
    let occ_b = board.black_pieces();
    let all_occ = occ_w | occ_b;
    let mut mob_kn_w=0; let mut mob_kn_b=0; let mut mob_bi_w=0; let mut mob_bi_b=0; let mut mob_r_w=0; let mut mob_r_b=0; let mut mob_q_w=0; let mut mob_q_b=0;
    // King ring bitboards
    let w_king_sq = board.white_king.trailing_zeros() as usize;
    let b_king_sq = board.black_king.trailing_zeros() as usize;
    let ring = |sq: usize| crate::attack_tables::get_attack_tables().king_attacks(sq) | (1u64<<sq);
    let w_ring = ring(w_king_sq); let b_ring = ring(b_king_sq);
    let mut kingring_attacks_w = 0; let mut kingring_attacks_b = 0;
    // Iterate pieces
    let mut bb;
    bb = board.white_knights; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; let sqs = crate::Square(sq/8, sq%8); let targets = bbm::knight_attacks_from(sqs) & !occ_w; mob_kn_w += targets.count_ones() as i32; kingring_attacks_w += (targets & b_ring).count_ones() as i32; }
    bb = board.black_knights; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; let sqs = crate::Square(sq/8, sq%8); let targets = bbm::knight_attacks_from(sqs) & !occ_b; mob_kn_b += targets.count_ones() as i32; kingring_attacks_b += (targets & w_ring).count_ones() as i32; }
    bb = board.white_bishops; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; let sqs = crate::Square(sq/8, sq%8); let targets = bbm::bishop_attacks_from(sqs, all_occ) & !occ_w; mob_bi_w += targets.count_ones() as i32; kingring_attacks_w += (targets & b_ring).count_ones() as i32; }
    bb = board.black_bishops; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; let sqs=crate::Square(sq/8,sq%8); let targets = bbm::bishop_attacks_from(sqs, all_occ) & !occ_b; mob_bi_b += targets.count_ones() as i32; kingring_attacks_b += (targets & w_ring).count_ones() as i32; }
    bb = board.white_rooks; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; let sqs=crate::Square(sq/8,sq%8); let targets = bbm::rook_attacks_from(sqs, all_occ) & !occ_w; mob_r_w += targets.count_ones() as i32; kingring_attacks_w += (targets & b_ring).count_ones() as i32; }
    bb = board.black_rooks; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; let sqs=crate::Square(sq/8,sq%8); let targets = bbm::rook_attacks_from(sqs, all_occ) & !occ_b; mob_r_b += targets.count_ones() as i32; kingring_attacks_b += (targets & w_ring).count_ones() as i32; }
    bb = board.white_queens; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; let sqs=crate::Square(sq/8,sq%8); let targets = (bbm::rook_attacks_from(sqs, all_occ)|bbm::bishop_attacks_from(sqs, all_occ)) & !occ_w; mob_q_w += targets.count_ones() as i32; kingring_attacks_w += (targets & b_ring).count_ones() as i32; }
    bb = board.black_queens; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; let sqs=crate::Square(sq/8,sq%8); let targets = (bbm::rook_attacks_from(sqs, all_occ)|bbm::bishop_attacks_from(sqs, all_occ)) & !occ_b; mob_q_b += targets.count_ones() as i32; kingring_attacks_b += (targets & w_ring).count_ones() as i32; }
    // Pawn structure counts
    let mut white_pawns_by_file=[0u8;8]; let mut black_pawns_by_file=[0u8;8];
    bb = board.white_pawns; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; white_pawns_by_file[sq%8]+=1; }
    bb = board.black_pawns; while bb!=0 { let sq=bb.trailing_zeros() as usize; bb&=bb-1; black_pawns_by_file[sq%8]+=1; }
    let mut isolated_w=0; let mut isolated_b=0; let mut doubled_w=0; let mut doubled_b=0; let mut backward_w=0; let mut backward_b=0; let mut passed_w=0; let mut passed_b=0;
    let file_mask = |f:usize| -> u64 {0x0101010101010101u64 << f};
    for f in 0..8 {
        if white_pawns_by_file[f]>1 { doubled_w += (white_pawns_by_file[f] as i32 -1); }
        if black_pawns_by_file[f]>1 { doubled_b += (black_pawns_by_file[f] as i32 -1); }
        let wl = if f>0 {white_pawns_by_file[f-1]} else {0}; let wr = if f<7 {white_pawns_by_file[f+1]} else {0};
        if white_pawns_by_file[f]>0 && wl==0 && wr==0 { isolated_w += 1; }
        let bl = if f>0 {black_pawns_by_file[f-1]} else {0}; let br = if f<7 {black_pawns_by_file[f+1]} else {0};
        if black_pawns_by_file[f]>0 && bl==0 && br==0 { isolated_b += 1; }
        // Passed/backward detection loops mirror evaluate
        let mut wp_file = board.white_pawns & file_mask(f); while wp_file!=0 { let sq=wp_file.trailing_zeros() as usize; wp_file&=wp_file-1; let rank=sq/8; let mut blocked=false; 'b: for cf in f.saturating_sub(1)..=usize::min(f+1,7){ let mut bp_scan=board.black_pawns & file_mask(cf); while bp_scan!=0 { let bsq=bp_scan.trailing_zeros() as usize; bp_scan&=bp_scan-1; if bsq/8 > rank { blocked=true; break 'b; } } } if !blocked { passed_w +=1; } if rank<7 { let forward_sq = sq + 8; let left_attack = if f>0 && forward_sq+7<64 { board.black_pawns & (1u64<<(forward_sq+7)) } else {0}; let right_attack = if f<7 && forward_sq+9<64 { board.black_pawns & (1u64<<(forward_sq+9)) } else {0}; if left_attack|right_attack!=0 { backward_w +=1; } } }
        let mut bp_file = board.black_pawns & file_mask(f); while bp_file!=0 { let sq=bp_file.trailing_zeros() as usize; bp_file&=bp_file-1; let rank=sq/8; let mut blocked=false; 'bw: for cf in f.saturating_sub(1)..=usize::min(f+1,7){ let mut wp_scan=board.white_pawns & file_mask(cf); while wp_scan!=0 { let wsq=wp_scan.trailing_zeros() as usize; wp_scan&=wp_scan-1; if wsq/8 < rank { blocked=true; break 'bw; } } } if !blocked { passed_b +=1; } if rank>0 { let forward_sq=sq-8; let left_attack = if f>0 && forward_sq>=9 { board.white_pawns & (1u64<<(forward_sq-9)) } else {0}; let right_attack = if f<7 && forward_sq>=7 { board.white_pawns & (1u64<<(forward_sq-7)) } else {0}; if left_attack|right_attack!=0 { backward_b +=1; } } }
    }
    (mob_kn_w,mob_kn_b,mob_bi_w,mob_bi_b,mob_r_w,mob_r_b,mob_q_w,mob_q_b,isolated_w,isolated_b,doubled_w,doubled_b,backward_w,backward_b,passed_w,passed_b,kingring_attacks_w,kingring_attacks_b)
}

pub fn extract_features(board: &Board) -> ExportedFeatures { extract_features_with_labels(board, None, None, None) }

pub fn extract_features_with_labels(board: &Board, stockfish_cp: Option<i32>, stockfish_mate: Option<i32>, game_result: Option<i8>) -> ExportedFeatures {
    let fen = board_to_fen(board);
    let br = board.evaluate_with_breakdown();
    let (mob_kn_w,mob_kn_b,mob_bi_w,mob_bi_b,mob_r_w,mob_r_b,mob_q_w,mob_q_b,isolated_w,isolated_b,doubled_w,doubled_b,backward_w,backward_b,passed_w,passed_b,kingring_attacks_w,kingring_attacks_b) = granular_counts(board);
    ExportedFeatures {
        fen,
        material: br.material,
        piece_square: br.piece_square,
        mobility: br.mobility,
        king_ring: br.king_ring,
        bishop_pair: br.bishop_pair,
        rook_files: br.rook_files,
        pawn_structure: br.pawn_structure,
        hanging: br.hanging,
        tempo: br.tempo,
        scaled_eval: br.scaled_total,
        pre_scale_eval: br.pre_scale_total,
        mob_kn_w,mob_kn_b,mob_bi_w,mob_bi_b,mob_r_w,mob_r_b,mob_q_w,mob_q_b,
        isolated_w,isolated_b,doubled_w,doubled_b,backward_w,backward_b,passed_w,passed_b,
        kingring_attacks_w,kingring_attacks_b,
        stockfish_cp,stockfish_mate,game_result,
    }
}

pub fn export_positions_to_csv<W: Write>(boards: &[Board], mut out: W, include_header: bool) -> std::io::Result<()> {
    if include_header { writeln!(out, "{}", ExportedFeatures::csv_header())?; }
    for b in boards { let f = extract_features(b); writeln!(out, "{}", f.to_csv_line())?; }
    Ok(())
}

pub fn export_labeled_positions_to_csv<W: Write>(boards: &[(Board, Option<i32>, Option<i32>, Option<i8>)], mut out: W, include_header: bool) -> std::io::Result<()> {
    if include_header { writeln!(out, "{}", ExportedFeatures::csv_header())?; }
    for (b, cp, mate, res) in boards { let f = extract_features_with_labels(b, *cp, *mate, *res); writeln!(out, "{}", f.to_csv_line())?; }
    Ok(())
}
