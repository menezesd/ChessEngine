//! Standalone UCI chess engine binary.
use chess_engine::{Position, chess_search::{Search, RootReport}, Move, Color, sq};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::io::{self, Write};
use std::thread;
use std::sync::{Arc, Mutex};

fn main() {
    let pos = Position::startpos();
    let search = Search::new();
    println!("id name CleanRoomEngine");
    println!("id author ChessEngineDev" );
    println!("uciok");
    let stdin = io::stdin();
    let shared_pos = Arc::new(Mutex::new(pos));
    let shared_search = Arc::new(Mutex::new(search));
    // Ponder state
    let mut last_ponder_move: Option<Move> = None; // predicted opponent reply
    let mut is_pondering = false;
    let mut ponder_start: Option<std::time::Instant> = None;
    // Time control context from last normal 'go'
    #[derive(Clone, Copy, Default)]
    #[allow(dead_code)] // Some fields may be unused depending on time control mode
    struct TimeParams { wtime: u64, btime: u64, winc: u64, binc: u64, movestogo: Option<u64>, alloc_ms: u64 }
    let mut last_time_params = TimeParams::default();
    let mut search_thread: Option<thread::JoinHandle<()>> = None;
    #[allow(unused_assignments)]
    let mut ponder_mode = false; // retained for future extended state handling
    #[allow(unused_assignments)]
    let mut infinite_mode = false;
    loop {
        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() { break; }
        let line = line.trim();
        if line=="quit" { break; }
        if line=="isready" { println!("readyok"); continue; }
        if line=="ucinewgame" { if let Ok(mut s)=shared_search.lock() { s.reset(); } if let Ok(mut p)=shared_pos.lock() { *p = Position::startpos(); } continue; }
        if line.starts_with("setoption") {
            // setoption name <Name> value <X>
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 && parts[1]=="name" {
                let name = parts[2];
                // Find 'value' index
                if let Some(val_idx) = parts.iter().position(|p| p.eq_ignore_ascii_case("value")) {
                    if val_idx+1 < parts.len() {
                        let val_str = parts[val_idx+1];
                        if let Ok(mut s)=shared_search.lock() {
                            if name.eq_ignore_ascii_case("MultiPV") { if let Ok(v)=val_str.parse::<usize>() { s.multipv = v.max(1).min(16); println!("info string set MultiPV {}", s.multipv);} }
                            else if name.eq_ignore_ascii_case("Hash") { if let Ok(mb)=val_str.parse::<usize>() { let new_mb = mb.max(1).min(4096); s.hash_mb = new_mb; s.tt.resize(new_mb); println!("info string set Hash {}", new_mb); } }
                            else if name.eq_ignore_ascii_case("Threads") { if let Ok(t)=val_str.parse::<usize>() { s.threads = t.max(1).min(32); println!("info string set Threads {}", s.threads); } }
                        }
                    }
                }
            }
            continue;
        }
        if line.starts_with("position") {
            // If a new position arrives while pondering, stop the ponder search (wrong prediction)
            if is_pondering { if let Ok(s)=shared_search.lock() { s.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);} if let Some(h)=search_thread.take(){ let _=h.join(); } is_pondering=false; last_ponder_move=None; }
            // Syntax: position [startpos | fen <fenstring>] [moves m1 m2 ...]
            let parts = line.split_whitespace().skip(1).collect::<Vec<_>>();
            if parts.is_empty() { continue; }
            let mut idx = 0;
            if parts[idx] == "startpos" { if let Ok(mut p)=shared_pos.lock() { *p = Position::startpos(); } idx +=1; }
            else if parts[idx] == "fen" {
                idx +=1; let mut fen_tokens = Vec::new();
                while idx < parts.len() && fen_tokens.len() < 6 && parts[idx] != "moves" { fen_tokens.push(parts[idx]); idx+=1; }
                if fen_tokens.len() >= 4 { let fen = fen_tokens.join(" "); if let Ok(pf)=Position::from_fen(&fen) { if let Ok(mut p)=shared_pos.lock() { *p=pf; } } }
            }
            if idx < parts.len() && parts[idx] == "moves" { idx+=1; while idx < parts.len() { if let Ok(mut p)=shared_pos.lock() { if let Some(mv)=parse_uci_move(&p, parts[idx]) { let _=p.make_move(&mv); } } idx+=1; } }
            continue;
        }
        if line=="stop" {
            if let Ok(s) = shared_search.lock() { s.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed); }
            if let Some(h) = search_thread.take() { let _=h.join(); }
            if let Ok(s) = shared_search.lock() { if let Some(b)=s.last_best_move { println!("bestmove {}", b); } else { println!("bestmove 0000"); } }
            last_ponder_move = None; is_pondering = false; /* ponder_mode flag not needed after stop */ continue;
        }
        if line=="ponderhit" { 
            // Convert ponder search into normal limited search automatically
            if is_pondering { 
                if let Ok(s)=shared_search.lock() { s.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed); }
                if let Some(h)=search_thread.take(){ let _=h.join(); }
                let elapsed = ponder_start.map(|st| st.elapsed().as_millis() as u64).unwrap_or(0);
                is_pondering=false; ponder_start=None; 
                // Allocate confirm search time: base slice from previous alloc minus half ponder elapsed (but not below 30ms)
                let base_slice = if last_time_params.alloc_ms>0 { last_time_params.alloc_ms } else { 1000 };
                let confirm_time = base_slice.saturating_sub(elapsed/2).clamp(30, base_slice);
                let pos_arc = Arc::clone(&shared_pos);
                let search_arc = Arc::clone(&shared_search);
                search_thread = Some(thread::spawn(move || {
                    if let (Ok(mut p), Ok(mut s)) = (pos_arc.lock(), search_arc.lock()) {
                        let best = s.think(&mut p, confirm_time, 64);
                        if let Some(b)=best { 
                            let ponder = if s.last_pv_line.len()>=2 { Some(s.last_pv_line[1]) } else { None };
                            if let Some(pm)=ponder { println!("bestmove {} ponder {}", b, pm); } else { println!("bestmove {}", b); }
                        } else { println!("bestmove 0000"); }
                    }
                }));
            }
            // After ponderhit, we are no longer in ponder pre-move state
            continue; }
        if line.starts_with("go ponder") {
            // GUI requests we start pondering (we just output a bestmove earlier with ponder move). Start search only if we have a predicted reply.
            if let Some(pm) = last_ponder_move {
                if let Ok(base_pos)=shared_pos.lock() {
                    // Make predicted opponent move on a clone position for ponder search
                    let mut clone = base_pos.clone();
                    let legal = clone.legal_moves();
                    if legal.contains(&pm) {
                        let _u = clone.make_move(&pm); // now our turn after predicted reply
                        // Dynamic ponder allocation: up to min(3*previous alloc, 1/4 of reported remaining time)
                        let our_time = if clone.side==Color::White { last_time_params.wtime } else { last_time_params.btime };
                        let base_slice = if last_time_params.alloc_ms>0 { last_time_params.alloc_ms } else { 1000 };
                        let ponder_cap_clock = if our_time>0 { our_time/4 } else { 30_000 };
                        let ponder_time = (base_slice*3).min(ponder_cap_clock).clamp(500, 60_000);
                        if let Some(h)=search_thread.take() { let _=h.join(); }
                        let search_arc = Arc::clone(&shared_search);
                        ponder_start = Some(std::time::Instant::now());
                        search_thread = Some(thread::spawn(move || {
                            if let Ok(mut s)=search_arc.lock() { s.think(&mut clone, ponder_time, 64); /* dynamic ponder cap */ }
                        }));
                        is_pondering = true;
                    } else { last_ponder_move=None; }
                }
            }
            continue; }
        if line.starts_with("go") {
            // Supported: go depth D | go movetime T | go wtime X btime Y [winc I] [binc I] [movestogo M]
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let mut time_ms: Option<u64> = None;
            let mut depth: Option<u8> = None;
            let mut wtime: Option<u64> = None; let mut btime: Option<u64> = None;
            let mut winc: u64 = 0; let mut binc: u64 = 0; let mut movestogo: Option<u64> = None;
            ponder_mode = false; infinite_mode = false;
            let mut i=1;
            while i < tokens.len() { match tokens[i] { "depth" => { i+=1; if i<tokens.len() { depth = tokens[i].parse::<u32>().ok().map(|d| d.min(255) as u8); } },
                                            "movetime" => { i+=1; if i<tokens.len() { time_ms = tokens[i].parse::<u64>().ok(); } },
                                            "wtime" => { i+=1; if i<tokens.len() { wtime = tokens[i].parse::<u64>().ok(); } },
                                            "btime" => { i+=1; if i<tokens.len() { btime = tokens[i].parse::<u64>().ok(); } },
                                            "winc" => { i+=1; if i<tokens.len() { winc = tokens[i].parse::<u64>().unwrap_or(0); } },
                                            "binc" => { i+=1; if i<tokens.len() { binc = tokens[i].parse::<u64>().unwrap_or(0); } },
                                            "movestogo" => { i+=1; if i<tokens.len() { movestogo = tokens[i].parse::<u64>().ok(); } },
                                            "ponder" => { ponder_mode = true; },
                                            "infinite" => { infinite_mode = true; },
                                            _ => {} }; i+=1; }
            // Determine time allocation
            let alloc_time; let max_depth;
            if infinite_mode { alloc_time = u64::MAX/4; max_depth = depth.unwrap_or(64); }
            else if let Some(mt)=time_ms { alloc_time = mt; max_depth = depth.unwrap_or(64); }
            else if depth.is_some() { alloc_time = 60_000; max_depth = depth.unwrap(); }
            else { // clock-based improved heuristic
                let (our_time, our_inc) = {
                    let p = shared_pos.lock().unwrap();
                    if p.side == Color::White { (wtime.unwrap_or(1000), winc) } else { (btime.unwrap_or(1000), binc) }
                };
                let mtg = movestogo.unwrap_or( (our_time / 15_000).clamp(10,60) );
                // base slice
                let base = our_time / mtg.max(1);
                // use a fraction of increment (80%)
                let inc_bonus = (our_inc as f64 * 0.8) as u64;
                // scale a bit deeper if large advantage in time
                alloc_time = (base + inc_bonus).min(our_time.saturating_sub(30)).max(15);
                max_depth = depth.unwrap_or(64);
            }
            // Spawn search threads (Lazy SMP if Threads>1)
            if let Some(h) = search_thread.take() { let _=h.join(); }
            let pos_clone_arc = Arc::clone(&shared_pos);
            let search_arc = Arc::clone(&shared_search);
            // Store time params for later ponder usage (bestmove extraction)
            last_time_params = TimeParams { wtime: wtime.unwrap_or(0), btime: btime.unwrap_or(0), winc, binc, movestogo, alloc_ms: alloc_time };
            let threads = { search_arc.lock().ok().map(|s| s.threads ).unwrap_or(1) }.max(1);
            if threads == 1 {
                let handle = thread::spawn(move || {
                    if let (Ok(mut p), Ok(mut s)) = (pos_clone_arc.lock(), search_arc.lock()) {
                        let local_best = s.think(&mut p, alloc_time, max_depth);
                        if !infinite_mode && !ponder_mode { 
                            if let Some(b)=local_best { 
                                let ponder = if s.last_pv_line.len() >= 2 { Some(s.last_pv_line[1]) } else { None };
                                if let Some(pm)=ponder { println!("bestmove {} ponder {}", b, pm); } else { println!("bestmove {}", b); }
                            } else { println!("bestmove 0000"); }
                        }
                    }
                });
                search_thread = Some(handle);
            } else {
                // Lazy SMP path
                let (tx, rx) = mpsc::channel::<RootReport>();
                let stop = Arc::new(AtomicBool::new(false));
                // Prepare shared TT sized as per current search setting
                let (hash_mb, multipv) = {
                    let s = search_arc.lock().unwrap(); (s.hash_mb, s.multipv)
                };
                let shared_tt = Arc::new(RwLock::new(chess_engine::chess_tt::TransTable::new(hash_mb)));
                // Seed TT by copying from main search (optional)
                if let Ok(s) = search_arc.lock() { if s.shared_tt.is_none() { /* no-op */ } }
                // Spawn workers
                let mut worker_handles = Vec::new();
                for id in 0..threads {
                    let txc = tx.clone();
                    let stopc = stop.clone();
                    let tt_c = shared_tt.clone();
                    let pos_c = { pos_clone_arc.lock().unwrap().clone() };
                    let multipv_primary = id == 0;
                    let aspiration_jitter = match id { 0 => 0, 1 => 8, 2 => -8, n => ((n as i32 % 5) - 2) * 6 };
                    worker_handles.push(thread::spawn(move || {
                        // Each worker owns its Search
                        let mut search = Search::with_shared_tt(tt_c, stopc.clone());
                        search.worker_id = id;
                        search.root_shuffle = id != 0; // primary keeps deterministic order
                        if multipv_primary { search.multipv = multipv; }
                        let mut pos_local = pos_c.clone();
                        let _best = search.think_worker(&mut pos_local, alloc_time, max_depth, Some(&txc), id, multipv_primary, aspiration_jitter);
                    }));
                }
                drop(tx);
                let start = std::time::Instant::now();
                // Aggregation loop
                let mut latest_depth_primary: u8 = 0;
                let mut latest_nodes: Vec<u64> = vec![0; threads];
                let deadline = start + std::time::Duration::from_millis(alloc_time);
                let mut bestmove_out: Option<Move> = None;
                let mut best_pv: Vec<Move> = Vec::new();
                let mut early_stop = false;
                while std::time::Instant::now() < deadline {
                    let timeout = deadline.saturating_duration_since(std::time::Instant::now());
                    if timeout.is_zero() { break; }
                    match rx.recv_timeout(timeout) {
                        Ok(rep) => {
                            // Track nodes
                            if rep.worker_id < latest_nodes.len() { latest_nodes[rep.worker_id] = rep.nodes; }
                            // Only print from primary
                            if rep.worker_id == 0 {
                                if rep.depth > latest_depth_primary && !rep.pv_sets.is_empty() {
                                    latest_depth_primary = rep.depth;
                                    let (sc, pv) = (&rep.pv_sets[0].0, &rep.pv_sets[0].1);
                                    let nps = if rep.elapsed_ms>0 { latest_nodes.iter().sum::<u64>()*1000 / rep.elapsed_ms } else { 0 };
                                    print!("info depth {} score {} nodes {} nps {} time {} pv", rep.depth, Search::uci_score(*sc), latest_nodes.iter().sum::<u64>(), nps, rep.elapsed_ms);
                                    for m in pv { print!(" {}", m); }
                                    println!();
                                    if let Some(mv) = pv.first() { bestmove_out = Some(*mv); best_pv = pv.clone(); }
                                    // Early-stop on mate score
                                    if sc.abs() > chess_engine::chess_search::EVAL_LIMIT {
                                        early_stop = true;
                                    }
                                }
                            }
                        }
                        Err(_) => break,
                    }
                    if early_stop { break; }
                }
                stop.store(true, Ordering::Relaxed);
                for h in worker_handles { let _ = h.join(); }
                // Emit bestmove
                if !infinite_mode && !ponder_mode {
                    if let Some(b)=bestmove_out { 
                        let ponder = if best_pv.len()>=2 { Some(best_pv[1]) } else { None };
                        if let Some(pm)=ponder { println!("bestmove {} ponder {}", b, pm); } else { println!("bestmove {}", b); }
                    } else {
                        println!("bestmove 0000");
                    }
                }
            }
            continue; }
        if line=="uci" { println!("id name CleanRoomEngine\nid author ChessEngineDev\nuciok"); continue; }
        print!("info string unknown_command {line}\n"); let _=io::stdout().flush();
    }
}

fn parse_uci_move(pos: &Position, s: &str) -> Option<Move> {
    if s.len() < 4 { return None; }
    let bytes = s.as_bytes();
    let from_file = bytes[0].wrapping_sub(b'a');
    let from_rank = bytes[1].wrapping_sub(b'1');
    let to_file = bytes[2].wrapping_sub(b'a');
    let to_rank = bytes[3].wrapping_sub(b'1');
    if from_file>7||from_rank>7||to_file>7||to_rank>7 { return None; }
    let from = sq(from_file, from_rank); let to = sq(to_file, to_rank);
    let legal = pos.legal_moves();
    for mv in legal { if mv.from==from && mv.to==to { if s.len()==5 { let promo = match &s[4..5] { "q"=>Some(chess_engine::Piece::Queen), "r"=>Some(chess_engine::Piece::Rook), "b"=>Some(chess_engine::Piece::Bishop), "n"=>Some(chess_engine::Piece::Knight), _=>None }; if promo.is_none() { continue; } return Some(Move{ promo, ..mv}); } return Some(mv); } }
    None
}
