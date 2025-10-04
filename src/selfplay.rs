use std::process::{Command, Stdio};
use std::io::{Write, BufRead, BufReader};
use crate::Board;

// Self-play / external Stockfish position generation with diversification and adjudication controls.
pub struct SelfPlayConfig {
    pub plies: usize,
    pub stockfish_path: String,
    pub sf_depth: u32,
    pub sf_depth_late: Option<u32>,        // optional deeper depth after a given ply
    pub late_depth_start_ply: usize,       // ply to start using sf_depth_late
    pub top_k_random: usize,               // pick randomly among top K static moves (>=1)
    pub adjudication_cp: i32,              // centipawn threshold for adjudication (|cp|)
    pub adjudication_repeats: u32,         // consecutive occurrences required
    pub min_plies_before_adjudication: usize, // minimum plies before adjudication starts
    pub random_opening_plies: usize,       // number of initial plies to select uniformly random (full width)
    pub clamp_cp: Option<i32>,             // optional clamp for stockfish_cp labels (abs)
    pub label_both_sides: bool,            // also label position after Stockfish move (doubles labels per full move)
}
impl Default for SelfPlayConfig { fn default() -> Self { SelfPlayConfig { plies: 60, stockfish_path: "/usr/games/stockfish".to_string(), sf_depth: 8, sf_depth_late: None, late_depth_start_ply: 60, top_k_random: 3, adjudication_cp: 1200, adjudication_repeats: 6, min_plies_before_adjudication: 50, random_opening_plies: 6, clamp_cp: Some(1200), label_both_sides: false } } }

pub fn generate_positions_vs_stockfish(cfg: &SelfPlayConfig) -> Vec<Board> {
    let mut boards = Vec::new();
    let mut board = Board::new();
    // Launch Stockfish
    let mut child = Command::new(&cfg.stockfish_path)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("launch stockfish");
    let mut sin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut send = |cmd: &str| { writeln!(sin, "{}", cmd).ok(); sin.flush().ok(); };
    send("uci");
    // Wait for uciok
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 { if line.contains("uciok") { break; } line.clear(); }
    send("isready"); line.clear(); while reader.read_line(&mut line).unwrap_or(0)>0 { if line.contains("readyok") { break;} line.clear(); }
    for _p in 0..cfg.plies { boards.push(board.clone()); // snapshot before move
        // Ask Stockfish for move if it's their turn every other half-move
        if _p % 2 == 1 { // Stockfish move
            send(&format!("position fen {}", crate::feature_export::extract_features(&board).fen));
            send(&format!("go depth {}", cfg.sf_depth));
            // Parse bestmove
            loop { line.clear(); if reader.read_line(&mut line).unwrap_or(0)==0 { break; } if line.starts_with("bestmove") { let parts: Vec<_> = line.split_whitespace().collect(); if parts.len()>=2 { if let Some(mv) = crate::uci::parse_uci_move(&mut board, parts[1]) { board.make_move(&mv); } } break; } }
        } else { // Our side: pick move randomly among top K static eval moves
            // Random opening phase: pick any legal move uniformly for first configured plies
            if _p < cfg.random_opening_plies { let moves = board.generate_moves(); if moves.is_empty() { break; } let idx = rand_index(moves.len() as u32) as usize; let mv = moves[idx]; let info = board.make_move(&mv); drop(info); }
            else {
                let mut scored: Vec<(i32, crate::Move)> = Vec::new();
                for mv in board.generate_moves() { let info = board.make_move(&mv); let sc = board.evaluate(); board.unmake_move(&mv, info); scored.push((sc, mv)); }
                if !scored.is_empty() { scored.sort_by_key(|x| -x.0); let k = cfg.top_k_random.max(1).min(scored.len()); let slice = &scored[..k]; let idx = if k==1 {0} else { rand_index(k as u32) as usize }; let mv = slice[idx].1; let info=board.make_move(&mv); drop(info); }
            }
        }
        if board.generate_moves().is_empty() { break; }
    }
    let _ = child.kill();
    boards
}

// Generate labeled positions with Stockfish centipawn scores (no mate parsing yet except simple bestmove search output lines if present).
pub fn generate_labeled_positions_vs_stockfish(cfg: &SelfPlayConfig) -> Vec<(Board, Option<i32>, Option<i32>, Option<i8>)> {
    let mut out: Vec<(Board, Option<i32>, Option<i32>, Option<i8>)> = Vec::new();
    let mut adjudication_counter: u32 = 0; // counts consecutive large-eval observations
    let mut last_cp: Option<i32> = None;
    let mut board = Board::new();
    let mut child = Command::new(&cfg.stockfish_path)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("launch stockfish");
    let mut sin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut send = |cmd: &str| { writeln!(sin, "{}", cmd).ok(); sin.flush().ok(); };
    send("uci");
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 { if line.contains("uciok") { break; } line.clear(); }
    send("isready"); line.clear(); while reader.read_line(&mut line).unwrap_or(0)>0 { if line.contains("readyok") { break;} line.clear(); }
    for ply in 0..cfg.plies {
        // Determine depth (dynamic late depth if configured)
        let depth_here = if ply >= cfg.late_depth_start_ply { cfg.sf_depth_late.unwrap_or(cfg.sf_depth) } else { cfg.sf_depth };
        // Evaluate current position with Stockfish to label before any move
        let fen = crate::feature_export::board_to_fen(&board);
        send(&format!("position fen {}", fen));
        send(&format!("go depth {}" , depth_here));
    let mut cp: Option<i32> = None; let mut mate: Option<i32> = None; let mut best_move: Option<String> = None;
        loop { line.clear(); if reader.read_line(&mut line).unwrap_or(0)==0 { break; }
            if line.starts_with("info ") && line.contains(" score ") {
                // parse score cp or mate
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(idx) = parts.iter().position(|p| *p=="score") { if idx+2 < parts.len() { if parts[idx+1]=="cp" { if let Ok(v)=parts[idx+2].parse::<i32>() { let mut val=v; if let Some(cl) = cfg.clamp_cp { if val>cl { val=cl; } else if val < -cl { val = -cl; } } cp=Some(val); } } else if parts[idx+1]=="mate" { if let Ok(v)=parts[idx+2].parse::<i32>() { mate=Some(v); } } } }
            }
            if line.starts_with("bestmove") { let seg: Vec<&str>=line.split_whitespace().collect(); if seg.len()>=2 { best_move=Some(seg[1].to_string()); } break; }
        }
        // Check adjudication: if eval suggests decisive advantage repeatedly
    if let Some(eval_cp) = cp { if eval_cp.abs() > cfg.adjudication_cp { adjudication_counter += 1; } else { adjudication_counter = 0; } last_cp = Some(eval_cp); }
        out.push((board.clone(), cp, mate, None));
        if let Some(bm) = best_move { if let Some(mv) = crate::uci::parse_uci_move(&mut board, &bm) { board.make_move(&mv); } else { break; } } else { break; }

        // Optionally label the position after Stockfish's move (before our reply)
        if cfg.label_both_sides {
            let post_fen = crate::feature_export::board_to_fen(&board);
            send(&format!("position fen {}", post_fen));
            send(&format!("go depth {}", depth_here));
            let mut cp2: Option<i32> = None; let mut mate2: Option<i32> = None; let mut line2 = String::new();
            loop { line2.clear(); if reader.read_line(&mut line2).unwrap_or(0)==0 { break; }
                if line2.starts_with("info ") && line2.contains(" score ") { let parts: Vec<&str>=line2.split_whitespace().collect(); if let Some(idx)=parts.iter().position(|p| *p=="score") { if idx+2<parts.len() { if parts[idx+1]=="cp" { if let Ok(v)=parts[idx+2].parse::<i32>() { let mut val=v; if let Some(cl)=cfg.clamp_cp { if val>cl { val=cl; } else if val < -cl { val=-cl; } } cp2=Some(val); } } else if parts[idx+1]=="mate" { if let Ok(v)=parts[idx+2].parse::<i32>() { mate2=Some(v); } } } } }
                if line2.starts_with("bestmove") { break; } // ignore bestmove for intermediate label
            }
            out.push((board.clone(), cp2, mate2, None));
        }
        if board.generate_moves().is_empty() { break; }
    // Our reply: random among top K
    if ply+1 < cfg.plies {
        if ply+1 < cfg.random_opening_plies { let moves = board.generate_moves(); if !moves.is_empty() { let idx = rand_index(moves.len() as u32) as usize; let mv = moves[idx]; let info = board.make_move(&mv); drop(info);} }
        else { let mut scored: Vec<(i32, crate::Move)> = Vec::new(); for mv in board.generate_moves() { let info=board.make_move(&mv); let sc=board.evaluate(); board.unmake_move(&mv, info); scored.push((sc,mv)); } if !scored.is_empty() { scored.sort_by_key(|x| -x.0); let k = cfg.top_k_random.max(1).min(scored.len()); let slice=&scored[..k]; let idx = if k==1 {0} else { rand_index(k as u32) as usize }; let mv = slice[idx].1; let info=board.make_move(&mv); drop(info);} }
    }
        if board.generate_moves().is_empty() { break; }
        if ply >= cfg.min_plies_before_adjudication && adjudication_counter >= cfg.adjudication_repeats { // adjudication trigger
            let result: i8 = if let Some(e) = last_cp { if e > 0 { 1 } else { -1 } } else { 0 };
            if let Some(last) = out.last_mut() { last.3 = Some(result); }
            break;
        }
    }
    let _ = child.kill();
    // If reach end without adjudication, assign draw (0) to last entry
    if let Some(last) = out.last_mut() { if last.3.is_none() { last.3 = Some(0); } }
    out
}

// Simple random index helper using xorshift (avoid pulling RNG repeatedly from rand crate overhead here)
fn rand_index(upper: u32) -> u32 { // upper >0
    use std::cell::RefCell; thread_local! { static STATE: RefCell<u64> = RefCell::new(0x9E3779B97F4A7C15); }
    STATE.with(|s| { let mut v = *s.borrow(); v ^= v<<7; v ^= v>>9; v = v.wrapping_mul(0x2545F4914F6CDD1D); *s.borrow_mut()=v; (v as u32)%upper })
}
