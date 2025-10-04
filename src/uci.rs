use crate::mvv_lva_score;
use crate::search::SearchEngine;
use crate::{Board, Move, TranspositionTable};
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

pub fn format_square(sq: crate::Square) -> String {
    format!("{}{}", (sq.1 as u8 + b'a') as char, sq.0 + 1)
}

pub fn format_uci_move(mv: &Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            crate::Piece::Queen => 'q',
            crate::Piece::Rook => 'r',
            crate::Piece::Bishop => 'b',
            crate::Piece::Knight => 'n',
            _ => '?',
        });
    }
    s
}

// Helpers local to this module
fn file_to_index(file: char) -> usize {
    file as usize - ('a' as usize)
}
fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('0' as usize) - 1
}

// Parses a move in UCI format (e.g., "e2e4", "e7e8q").
pub fn parse_uci_move(board: &mut Board, uci_string: &str) -> Option<Move> {
    if uci_string.len() < 4 || uci_string.len() > 5 {
        return None;
    }
    let mut chars = uci_string.chars();
    let from_file = chars.next()?;
    let from_rank = chars.next()?;
    let to_file = chars.next()?;
    let to_rank = chars.next()?;
    if !('a'..='h').contains(&from_file)
        || !('1'..='8').contains(&from_rank)
        || !('a'..='h').contains(&to_file)
        || !('1'..='8').contains(&to_rank)
    {
        return None;
    }
    let from_sq = crate::Square(rank_to_index(from_rank), file_to_index(from_file));
    let to_sq = crate::Square(rank_to_index(to_rank), file_to_index(to_file));
    let promotion_piece = match chars.next() {
        Some('q') => Some(crate::Piece::Queen),
        Some('r') => Some(crate::Piece::Rook),
        Some('b') => Some(crate::Piece::Bishop),
        Some('n') => Some(crate::Piece::Knight),
        Some(_) => return None,
        None => None,
    };
    let legal_moves = board.generate_moves();
    for legal_move in legal_moves {
        if legal_move.from == from_sq && legal_move.to == to_sq {
            if legal_move.promotion == promotion_piece {
                return Some(legal_move.clone());
            } else if promotion_piece.is_none() && legal_move.promotion.is_none() {
                return Some(legal_move.clone());
            }
        }
    }
    None
}

pub fn parse_position_command(board: &mut Board, parts: &[&str]) {
    let mut i = 1;
    if i < parts.len() && parts[i] == "startpos" {
        *board = Board::new();
        i += 1;
    } else if i < parts.len() && parts[i] == "fen" {
        let fen = parts[i + 1..i + 7].join(" ");
        *board = Board::from_fen(&fen);
        i += 7;
    }
    if i < parts.len() && parts[i] == "moves" {
        i += 1;
        while i < parts.len() {
            if let Some(mv) = parse_uci_move(board, parts[i]) {
                board.make_move(&mv);
            }
            i += 1;
        }
    }
}

pub fn find_best_move_with_time(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_time: Duration,
    start_time: Instant,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut depth = 1;
    let mut last_depth_time = Duration::from_millis(1);
    const SAFETY_MARGIN: Duration = Duration::from_millis(5);
    const TIME_GROWTH_FACTOR: f32 = 2.0;
    let mut engine = SearchEngine::new();
    // Provide a soft deadline so search can abort cleanly before the hard limit
    let soft = max_time.saturating_sub(SAFETY_MARGIN);
    engine.timer.deadline = Some(start_time + soft);
    let mut best_score = -crate::MATE_SCORE * 2;
    while start_time.elapsed() + SAFETY_MARGIN < max_time {
        let elapsed = start_time.elapsed();
        let time_remaining = max_time.checked_sub(elapsed).unwrap_or_default();
        if last_depth_time.mul_f32(TIME_GROWTH_FACTOR) + SAFETY_MARGIN > time_remaining {
            break;
        }
        let depth_start = Instant::now();
        let nodes_before = engine.timer.node_count;
        let mut legal_moves = board.generate_moves();
        if legal_moves.is_empty() {
            return None;
        }
        if legal_moves.len() == 1 {
            return Some(legal_moves[0]);
        }
        legal_moves.sort_by_key(|m| -mvv_lva_score(m, board));
        if let Some(entry) = tt.probe(board.hash) {
            if let Some(hm) = &entry.best_move {
                if let Some(pos) = legal_moves.iter().position(|m| m == hm) {
                    legal_moves.swap(0, pos);
                }
            }
        }
        // Adaptive aspiration window
        let (mut alpha, mut beta) = if depth <= 3 || best_move.is_none() {
            // Wide window for shallow depths or first search
            (-crate::MATE_SCORE * 2, crate::MATE_SCORE * 2)
        } else {
            // Narrow window based on previous score and depth
            let window = if depth <= 6 { 25 } else if depth <= 10 { 50 } else { 75 };
            (best_score - window, best_score + window)
        };
        
        let mut searches = 0;
        let mut new_best_move: Option<Move>;
        let mut fail_high_count = 0;
        let mut fail_low_count = 0;
        
        loop {
            searches += 1;
            new_best_move = None;
            best_score = -crate::MATE_SCORE * 2;
            
            for m in &legal_moves {
                if start_time.elapsed() + SAFETY_MARGIN >= max_time { break; }
                let info = board.make_move(m);
                let mut sc = crate::search::SearchContext::new();
                let score = -engine.search(board, &mut sc, 1, -beta, -alpha, depth - 1, false, false);
                board.unmake_move(m, info);
                if score > best_score { best_score = score; new_best_move = Some(*m); }
                alpha = alpha.max(best_score);
                if alpha >= beta { break; }
            }
            
            if start_time.elapsed() + SAFETY_MARGIN >= max_time { break; }
            
            // Check aspiration window bounds
            if best_score <= alpha {
                // Fail low - widen down
                fail_low_count += 1;
                let expand = if fail_low_count == 1 { 75 } else { 200 };
                alpha = best_score - expand;
                if searches >= 3 { alpha = -crate::MATE_SCORE * 2; }
            } else if best_score >= beta {
                // Fail high - widen up
                fail_high_count += 1;
                let expand = if fail_high_count == 1 { 75 } else { 200 };
                beta = best_score + expand;
                if searches >= 3 { beta = crate::MATE_SCORE * 2; }
            } else {
                // Score within window - success
                break;
            }
            
            if searches >= 4 {
                // Give up on aspiration after too many searches
                alpha = -crate::MATE_SCORE * 2;
                beta = crate::MATE_SCORE * 2;
            }
            
            // Move best move to front for re-search
            if let Some(mv) = new_best_move {
                if let Some(pos) = legal_moves.iter().position(|m| *m == mv) {
                    legal_moves.swap(0, pos);
                }
            }
        }
        if start_time.elapsed() + SAFETY_MARGIN < max_time {
            best_move = new_best_move;
            last_depth_time = depth_start.elapsed();
            // History aging is handled internally by the SearchEngine
            // Print UCI info for this completed depth
            let depth_time = last_depth_time.as_millis().max(1) as u128;
            let nodes = (engine.timer.node_count - nodes_before) as u128;
            let nps = (nodes * 1000) / depth_time;
            let pv = board.extract_pv(tt, 30);
            let pv_str = pv.iter().map(|m| format_uci_move(m)).collect::<Vec<_>>().join(" ");
            println!("info depth {} score cp {} time {} nodes {} nps {} pv {}",
                depth,
                best_score,
                last_depth_time.as_millis(),
                nodes,
                nps,
                pv_str);
            depth += 1;
        } else {
            // Time ran out mid-depth; keep the best move found in this iteration.
            if best_move.is_none() { best_move = new_best_move; }
            break;
        }
    }
    best_move
}

pub fn find_best_move_fixed_depth(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_depth: u32,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut engine = SearchEngine::new();
    let mut best_score = -crate::MATE_SCORE * 2;
    for depth in 1..=max_depth {
        let nodes_before = engine.timer.node_count;
        let mut legal_moves = board.generate_moves();
        if legal_moves.is_empty() { return None; }
        if legal_moves.len() == 1 { return Some(legal_moves[0]); }
        legal_moves.sort_by_key(|m| -mvv_lva_score(m, board));
        if let Some(entry) = tt.probe(board.hash) { if let Some(hm) = &entry.best_move { if let Some(pos) = legal_moves.iter().position(|m| m == hm) { legal_moves.swap(0, pos); } } }
        let mut alpha = -crate::MATE_SCORE * 2;
        let mut beta = crate::MATE_SCORE * 2;
        best_score = -crate::MATE_SCORE * 2;
        let mut new_best_move: Option<Move> = None;
        for m in &legal_moves {
            let info = board.make_move(m);
            let mut sc = crate::search::SearchContext::new();
            let score = -engine.search(board, &mut sc, 1, -beta, -alpha, depth - 1, false, false);
            board.unmake_move(m, info);
            if score > best_score { best_score = score; new_best_move = Some(*m); }
            alpha = alpha.max(best_score);
            if alpha >= beta { break; }
        }
        best_move = new_best_move;
        // Per-depth info
        let nodes = (engine.timer.node_count - nodes_before) as u128;
        // No wall-clock here; report time as 0 and nps as nodes (pseudo)
        let pv = board.extract_pv(tt, 30);
        let pv_str = pv.iter().map(|m| format_uci_move(m)).collect::<Vec<_>>().join(" ");
        println!("info depth {} score cp {} time {} nodes {} nps {} pv {}",
            depth,
            best_score,
            0,
            nodes,
            nodes,
            pv_str);
    }
    best_move
}

pub fn run() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut board = Board::new();
    let mut tt = TranspositionTable::new(1024);
    let mut time_left = Duration::from_secs(5);
    let mut inc = Duration::ZERO;
    let mut movetime: Option<Duration> = None;
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "uci" => {
                println!("id name MyRustEngine");
                println!("id author Dean Menezes");
                println!("option name Hash type spin default 1024 min 1 max 8192");
                println!("option name Clear Hash type button");
                println!("uciok");
            }
            "isready" => {
                println!("readyok");
            }
            "testsimple" => {
                println!("Testing advanced search:");
                let mut engine = SearchEngine::new();
                let time_limit = Some(std::time::Instant::now() + std::time::Duration::from_millis(1000));
                let best = engine.think(&mut board, time_limit);
                println!("Search result: {}", format_uci_move(&best));
            }
            "eval" => {
                let score = board.evaluate();
                println!("Static evaluation: {}", score);
            }
            "evalbreakdown" => {
                let br = board.evaluate_with_breakdown();
                let feats = crate::feature_export::extract_features(&board); // includes granular counts
                println!("Eval breakdown: material={} pst={} mobility={} king_ring={} bishop_pair={} rook_files={} pawn_structure={} hanging={} tempo={} pre_scale={} scaled={}",
                    br.material, br.piece_square, br.mobility, br.king_ring, br.bishop_pair, br.rook_files, br.pawn_structure, br.hanging, br.tempo, br.pre_scale_total, br.scaled_total);
                println!("Granular: mob_kn_w={} mob_kn_b={} mob_bi_w={} mob_bi_b={} mob_r_w={} mob_r_b={} mob_q_w={} mob_q_b={} isolated_w={} isolated_b={} doubled_w={} doubled_b={} backward_w={} backward_b={} passed_w={} passed_b={} kingring_attacks_w={} kingring_attacks_b={}",
                    feats.mob_kn_w,feats.mob_kn_b,feats.mob_bi_w,feats.mob_bi_b,feats.mob_r_w,feats.mob_r_b,feats.mob_q_w,feats.mob_q_b,feats.isolated_w,feats.isolated_b,feats.doubled_w,feats.doubled_b,feats.backward_w,feats.backward_b,feats.passed_w,feats.passed_b,feats.kingring_attacks_w,feats.kingring_attacks_b);
            }
            "dumpfeatures" => {
                // dumpfeatures json|csv (default json)
                let format = parts.get(1).copied().unwrap_or("json").to_lowercase();
                let f = crate::feature_export::extract_features(&board);
                if format=="csv" {
                    println!("{}", crate::feature_export::ExportedFeatures::csv_header());
                    println!("{}", f.to_csv_line());
                } else {
                    // Manual JSON serialization to avoid adding serde dependency
                    println!("{{\n  \"fen\": \"{}\",\n  \"material\": {},\n  \"piece_square\": {},\n  \"mobility\": {},\n  \"king_ring\": {},\n  \"bishop_pair\": {},\n  \"rook_files\": {},\n  \"pawn_structure\": {},\n  \"hanging\": {},\n  \"tempo\": {},\n  \"scaled_eval\": {},\n  \"pre_scale_eval\": {},\n  \"mob_kn_w\": {}, \"mob_kn_b\": {}, \"mob_bi_w\": {}, \"mob_bi_b\": {}, \"mob_r_w\": {}, \"mob_r_b\": {}, \"mob_q_w\": {}, \"mob_q_b\": {},\n  \"isolated_w\": {}, \"isolated_b\": {}, \"doubled_w\": {}, \"doubled_b\": {}, \"backward_w\": {}, \"backward_b\": {}, \"passed_w\": {}, \"passed_b\": {},\n  \"kingring_attacks_w\": {}, \"kingring_attacks_b\": {}\n}}",
                        f.fen,f.material,f.piece_square,f.mobility,f.king_ring,f.bishop_pair,f.rook_files,f.pawn_structure,f.hanging,f.tempo,f.scaled_eval,f.pre_scale_eval,
                        f.mob_kn_w,f.mob_kn_b,f.mob_bi_w,f.mob_bi_b,f.mob_r_w,f.mob_r_b,f.mob_q_w,f.mob_q_b,
                        f.isolated_w,f.isolated_b,f.doubled_w,f.doubled_b,f.backward_w,f.backward_b,f.passed_w,f.passed_b,
                        f.kingring_attacks_w,f.kingring_attacks_b);
                }
            }
            "testmoves" => {
                println!("Testing individual moves:");
                let moves = board.generate_moves();
                for (i, m) in moves.iter().take(5).enumerate() {
                    let info = board.make_move(m);
                    let score = -board.evaluate();
                    board.unmake_move(m, info);
                    println!("Move {}: {:?} -> Score: {}", i+1, m, score);
                }
            }
            "testintegrity" => {
                println!("Testing make/unmake integrity:");
                let original_hash = board.hash;
                let original_eval = board.evaluate();
                println!("Original - Hash: {}, Eval: {}", original_hash, original_eval);
                
                let moves = board.generate_moves();
                for (i, m) in moves.iter().take(3).enumerate() {
                    println!("Testing move {}: {:?}", i+1, m);
                    let info = board.make_move(m);
                    let after_make_hash = board.hash;
                    let after_make_eval = board.evaluate();
                    println!("  After make - Hash: {}, Eval: {}", after_make_hash, after_make_eval);
                    
                    board.unmake_move(m, info);
                    let after_unmake_hash = board.hash;
                    let after_unmake_eval = board.evaluate();
                    println!("  After unmake - Hash: {}, Eval: {}", after_unmake_hash, after_unmake_eval);
                    
                    if original_hash != after_unmake_hash {
                        println!("  ❌ HASH CORRUPTION!");
                    }
                    if original_eval != after_unmake_eval {
                        println!("  ❌ EVAL CORRUPTION!");
                    }
                    if original_hash == after_unmake_hash && original_eval == after_unmake_eval {
                        println!("  ✅ Make/unmake OK");
                    }
                }
            }
            "debugsearch" => {
                println!("Debug advanced search:");
                let mut engine = SearchEngine::new();
                let time_limit = Some(std::time::Instant::now() + std::time::Duration::from_millis(2000));
                let _best = engine.think(&mut board, time_limit);
                println!("Debug search completed");
            }
            "listmoves" => {
                println!("All legal moves from starting position:");
                let moves = board.generate_moves();
                for (i, m) in moves.iter().enumerate() {
                    println!("Move {}: {:?}", i+1, m);
                    if i >= 19 { break; } // Show first 20 moves
                }
                println!("Total moves: {}", moves.len());
            }
            "ucinewgame" => {
                board = Board::new();
                tt.clear();
            }
            "setoption" => {
                let mut i = 1;
                let mut name: Option<String> = None;
                let mut value: Option<String> = None;
                while i < parts.len() {
                    match parts[i] {
                        "name" => {
                            i += 1;
                            let mut name_tokens = Vec::new();
                            while i < parts.len() && parts[i] != "value" {
                                name_tokens.push(parts[i]);
                                i += 1;
                            }
                            name = Some(name_tokens.join(" "));
                        }
                        "value" => {
                            i += 1;
                            let mut value_tokens = Vec::new();
                            while i < parts.len() {
                                value_tokens.push(parts[i]);
                                i += 1;
                            }
                            value = Some(value_tokens.join(" "));
                        }
                        _ => i += 1,
                    }
                }
                if let Some(n) = name {
                    if n.eq_ignore_ascii_case("Hash") {
                        if let Some(v) = value {
                            if let Ok(mb) = v.parse::<usize>() {
                                tt.resize(mb);
                            }
                        }
                    } else if n.eq_ignore_ascii_case("Clear Hash") {
                        tt.clear();
                    }
                }
            }
            "position" => {
                parse_position_command(&mut board, &parts);
            }
            "clearhash" | "clear" => {
                tt.clear();
            }
            "go" => {
                let mut i = 1;
                let mut fixed_depth: Option<u32> = None;
                while i < parts.len() {
                    match parts[i] {
                        "wtime" if board.white_to_move => {
                            time_left = Duration::from_millis(parts[i + 1].parse().unwrap_or(1000));
                            i += 2;
                        }
                        "btime" if !board.white_to_move => {
                            time_left = Duration::from_millis(parts[i + 1].parse().unwrap_or(1000));
                            i += 2;
                        }
                        "winc" if board.white_to_move => {
                            inc = Duration::from_millis(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "binc" if !board.white_to_move => {
                            inc = Duration::from_millis(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "movetime" => {
                            movetime =
                                Some(Duration::from_millis(parts[i + 1].parse().unwrap_or(100)));
                            i += 2;
                        }
                        "depth" => {
                            fixed_depth = parts.get(i + 1).and_then(|s| s.parse::<u32>().ok());
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }
                // Start a new search: bump TT generation.
                tt.start_new_search();
                let mut best_move: Option<Move> = None;
                let mut best_score: i32 = 0;
                if let Some(_d) = fixed_depth {
                    // Use search engine for fixed depth (with time limit)
                    let mut engine = SearchEngine::new();
                    let time_limit = Some(Instant::now() + std::time::Duration::from_millis(5000));
                    let mv = engine.think(&mut board, time_limit);
                    best_score = board.evaluate();
                    best_move = Some(mv);
                } else {
                    let max_time = movetime.unwrap_or_else(|| time_left / 30 + inc);
                    let start = Instant::now();
                    best_move = find_best_move_with_time(&mut board, &mut tt, max_time, start);
                    if let Some(ref mv) = best_move {
                        let info = board.make_move(mv);
                        best_score = board.evaluate();
                        board.unmake_move(mv, info);
                    }
                }
                // Always print a final info score line before bestmove
                if let Some(ref mv) = best_move {
                    println!("info score cp {} pv {}", best_score, format_uci_move(mv));
                    println!("bestmove {}", format_uci_move(mv));
                } else {
                    println!("info score cp 0 pv");
                    println!("bestmove 0000");
                }
            }
            "quit" => break,
            _ => {}
        }
        stdout.flush().unwrap();
    }
}
