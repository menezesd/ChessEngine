use crate::transposition_table::TranspositionTable;
use crate::types::{format_square, Move};
use crate::board::Board;
use crate::ordering::{OrderingContext, order_moves};
use crate::zobrist::ZOBRIST;
// Local copy of material values (MG) for P, N, B, R, Q, K so we can compute a
// simple weighted-material sum without depending on public eval symbols.
const MATERIAL_MG: [i32; 6] = [82, 337, 365, 477, 1025, 20000];
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::sync::mpsc::Sender;
use crate::uci_info;
use crate::transposition_table::BoundType;
use std::sync::atomic::{AtomicI32, Ordering as AtomicOrdering};
use crate::see::see_capture;

pub fn negamax(
    board: &mut Board,
    tt: &mut TranspositionTable,
    depth: u32,
    mut alpha: i32,
    mut beta: i32,
    moves_buf: &mut Vec<Move>,
    ctx: &mut OrderingContext,
) -> i32 {
    let original_alpha = alpha;
    let current_hash = board.hash;

    // TT probe
    let mut hash_move: Option<Move> = None;
    if let Some(entry) = tt.probe(current_hash) {
        if entry.depth >= depth {
            match entry.bound_type {
                BoundType::Exact => return entry.score,
                BoundType::LowerBound => alpha = alpha.max(entry.score),
                BoundType::UpperBound => beta = beta.min(entry.score),
            }
            if alpha >= beta {
                return entry.score;
            }
        }
        hash_move = entry.best_move;
    }

    if crate::search_control::should_stop() {
        return 0;
    }

    if board.is_draw() {
        return 0;
    }

    crate::search_control::node_visited();

    if depth == 0 {
        return quiesce(board, alpha, beta, moves_buf, ctx);
    }

    // Null-move pruning: if not in check and depth is sufficient, try a reduced-depth null move.
    // Conditions: depth >= 3 and not in check and not a draw. We set reduction R=2.
    if depth >= 3 {
        let current_color = board.current_color();
        // Don't use null-move in low-material/endgame positions where zugzwang risk is high.
        if !board.is_in_check(current_color) && should_use_null_move(board) {
            // Make a null move: flip side and clear en-passant target (save/restore state)
            let saved_ep = board.en_passant_target;
            let saved_hash = board.hash;
            board.en_passant_target = None;
            board.white_to_move = !board.white_to_move;
            board.hash ^= ZOBRIST.black_to_move_key;

            // Reduced depth
            let r = 2u32;
            let score = -negamax(board, tt, depth - 1 - r, -beta, -beta + 1, moves_buf, ctx);

            // Undo null move
            board.white_to_move = !board.white_to_move;
            board.en_passant_target = saved_ep;
            board.hash = saved_hash;

            if score >= beta {
                // Verification search: full-window search at depth-1 to avoid zugzwang false cutoffs
                let ver_score = -negamax(board, tt, depth - 1, -beta, -alpha, moves_buf, ctx);
                if ver_score >= beta {
                    return ver_score;
                }
                // Otherwise continue searching normally
            }
        }
    }

    moves_buf.clear();
    board.generate_moves_into(moves_buf);
    // Use ordering heuristics (TT move already extracted above)
    order_moves(ctx, board, &mut moves_buf[..], depth as usize, hash_move);

    if moves_buf.is_empty() {
        let current_color = board.current_color();
        return if board.is_in_check(current_color) {
            -(crate::constants::MATE_SCORE - (100 - depth as i32))
        } else {
            0
        };
    }

    if let Some(hm) = &hash_move {
        if let Some(pos) = moves_buf.iter().position(|m| m == hm) {
            moves_buf.swap(0, pos);
        }
    }

    let mut best_score: i32 = -crate::constants::MATE_SCORE * 2;
    let mut best_move_found: Option<Move> = None;

    let mut child_buf: Vec<Move> = Vec::new();
    for (i, m) in moves_buf.iter().enumerate() {
        if crate::search_control::should_stop() {
            break;
        }
        // Capture attacker piece type before making the move (board state will change)
        let attacker_piece = board.piece_at(m.from).map(|(_c, p)| p);
        let info = board.make_move(m);
        let score = if i == 0 {
            -negamax(board, tt, depth - 1, -beta, -alpha, &mut child_buf, ctx)
        } else {
            let mut score = -negamax(board, tt, depth - 1, -alpha - 1, -alpha, &mut child_buf, ctx);
            if score > alpha && score < beta {
                score = -negamax(board, tt, depth - 1, -beta, -alpha, &mut child_buf, ctx);
            }
            score
        };
        board.unmake_move(m, info);

        // If this move improved best_score, record history for non-capture moves
        if score > best_score {
            best_score = score;
            best_move_found = Some(*m);
            if m.captured_piece.is_none() {
                if let Some(piece) = attacker_piece {
                    // small increment for history
                    ctx.record_history(piece, m.from.0 as u8, m.to.0 as u8, 1);
                }
            }
        }

        alpha = alpha.max(best_score);
        // On beta cutoff, record killer for non-captures and boost history
        if alpha >= beta {
            if m.captured_piece.is_none() {
                ctx.record_killer(depth as usize, *m);
                if let Some(piece) = attacker_piece {
                    ctx.record_history(piece, m.from.0 as u8, m.to.0 as u8, 32);
                }
            }
            break;
        }
    }

    let bound_type = if best_score <= original_alpha {
        BoundType::UpperBound
    } else if best_score >= beta {
        BoundType::LowerBound
    } else {
        BoundType::Exact
    };

    tt.store(current_hash, depth, best_score, bound_type, best_move_found);

    best_score
}

/// Simple heuristic that decides whether null-move pruning is safe in the
/// current position. We conservatively disable null-move in reduced-material
/// positions where only kings and pawns (or very little non-pawn material)
/// remain to avoid zugzwang-related false cutoffs.
fn should_use_null_move(board: &Board) -> bool {
    // First, check if a material threshold override is configured. We use a
    // global atomic i32 where negative value means "no threshold configured".
    if let Some(threshold) = get_nullmove_material_threshold() {
        // Compute weighted non-pawn material using MATERIAL_MG for pieces 1..4
        let mut mat: i32 = 0;
        for piece_idx in 1..5usize {
            let cnt = (board.bitboards[0][piece_idx].count_ones()
                + board.bitboards[1][piece_idx].count_ones()) as i32;
            mat += cnt * MATERIAL_MG[piece_idx];
        }
        // If total non-pawn material is below or equal to threshold, disable null-move
        return mat > threshold;
    }

    // Fallback conservative rule: count non-pawn pieces (N,B,R,Q) for both sides.
    let mut non_pawn_count = 0u32;
    for piece_idx in 1..5 {
        let ww = board.bitboards[0][piece_idx];
        let bb = board.bitboards[1][piece_idx];
        non_pawn_count += ww.count_ones();
        non_pawn_count += bb.count_ones();
    }

    // If there is very little non-pawn material (e.g., <= 1 piece besides kings),
    // disable null-move to avoid zugzwang issues.
    non_pawn_count > 1
}

static NULLMOVE_MATERIAL_THRESHOLD: AtomicI32 = AtomicI32::new(-1);

/// Set a weighted-material threshold (in centipawns) below which null-move
/// pruning will be disabled. Use `None` to clear the threshold and fall back
/// to the conservative piece-count heuristic.
pub fn set_nullmove_material_threshold(opt: Option<i32>) {
    let v = opt.unwrap_or(-1);
    NULLMOVE_MATERIAL_THRESHOLD.store(v, AtomicOrdering::SeqCst);
}

/// Get the configured material threshold, or `None` if not set.
pub fn get_nullmove_material_threshold() -> Option<i32> {
    let v = NULLMOVE_MATERIAL_THRESHOLD.load(AtomicOrdering::SeqCst);
    if v < 0 { None } else { Some(v) }
}

/// Quiescence search extracted from Board::quiesce
pub fn quiesce(
    board: &mut Board,
    mut alpha: i32,
    beta: i32,
    moves_buf: &mut Vec<Move>,
    ctx: &mut OrderingContext,
) -> i32 {
    let stand_pat_score = board.evaluate();
    if board.is_draw() {
        return 0;
    }
    if crate::search_control::should_stop() {
        return stand_pat_score;
    }
    crate::search_control::node_visited();
    if stand_pat_score >= beta {
        return beta;
    }
    alpha = alpha.max(stand_pat_score);

    moves_buf.clear();
    board.generate_tactical_moves_into(moves_buf);
    // Use SEE to prune obviously losing captures and order by SEE value
    moves_buf.retain(|m| {
        let s = see_capture(board, m);
        s >= 0
    });
    moves_buf.sort_by_key(|m| -see_capture(board, m));
    order_moves(ctx, board, &mut moves_buf[..], 0, None);

    let mut best_score = stand_pat_score;
    let tactical_moves = moves_buf.clone();
    for m in tactical_moves {
        if crate::search_control::should_stop() {
            break;
        }
    let info = board.make_move(&m);
    let score = -quiesce(board, -beta, -alpha, moves_buf, ctx);
    board.unmake_move(&m, info);

        best_score = best_score.max(score);
        alpha = alpha.max(best_score);
        if alpha >= beta {
            break;
        }
    }

    alpha
}

/// Iterative deepening search driver which publishes `uci_info::Info` messages to `info_sender`
/// and updates an optional sink with intermediate best moves.
#[allow(dead_code)]
pub fn iterative_deepening_with_sink(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_depth: u32,
    sink: Option<Arc<Mutex<Option<Move>>>>,
    info_sender: Option<Sender<uci_info::Info>>,
    is_ponder: bool,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut best_score: i32;

    let mut legal_moves = Vec::new();
    board.generate_moves_into(&mut legal_moves);
    if legal_moves.is_empty() {
        return None;
    }
    if legal_moves.len() == 1 {
        return Some(legal_moves[0]);
    }
    let mut root_moves = legal_moves;

    let search_start = Instant::now();
    let mut prev_score: Option<i32> = None;
    const ASPIRATION_WINDOW: i32 = 50; // centipawns
    for depth in 1..=max_depth {
        // Build aspiration window from previous score if available
        let mut window = None;
        if let Some(ps) = prev_score {
            let a = ps.saturating_sub(ASPIRATION_WINDOW);
            let b = ps.saturating_add(ASPIRATION_WINDOW);
            window = Some((a, b));
        }

        // First try with aspiration window (if any)
        let (mut mv_opt, mut score, mut completed) = board.run_root_search(
            tt,
            depth,
            &mut root_moves,
            || crate::search_control::should_stop(),
            window,
        );

        // If aspiration failed (score outside window), re-search with full window
        if let Some((a, b)) = window {
            if completed && (score <= a || score >= b) {
                let (mv_opt2, score2, completed2) = board.run_root_search(
                    tt,
                    depth,
                    &mut root_moves,
                    || crate::search_control::should_stop(),
                    None,
                );
                mv_opt = mv_opt2;
                score = score2;
                completed = completed2;
            }
        }

        if completed {
            if let Some(mv) = mv_opt {
                best_move = Some(mv);
                best_score = score;

                if let Some(ref s) = sink {
                    let mut lock = match s.lock() {
                        Ok(g) => g,
                        Err(poisoned) => {
                            eprintln!("warning: sink mutex poisoned, recovering");
                            poisoned.into_inner()
                        }
                    };
                    *lock = best_move;
                }

                if let Some(ref sender) = info_sender {
                    let nodes_total = crate::search_control::get_node_count();
                    let elapsed_ms = search_start.elapsed().as_millis();
                    let nps = if elapsed_ms > 0 {
                        Some(((nodes_total as u128 * 1000) / elapsed_ms) as u64)
                    } else {
                        None
                    };
                    let pv = build_pv_from_tt(tt, board.hash);
                    let mut info = uci_info::Info {
                        depth: Some(depth),
                        nodes: Some(nodes_total),
                        nps,
                        time_ms: Some(elapsed_ms),
                        score_cp: None,
                        score_mate: None,
                        pv: Some(pv),
                        seldepth: Some(depth),
                        ponder: None,
                    };
                    if best_score.abs() > (crate::constants::MATE_SCORE / 2) {
                        let mate_in = (crate::constants::MATE_SCORE - best_score.abs() + 1) / 2;
                        info.score_mate = Some(mate_in);
                    } else {
                        info.score_cp = Some(best_score);
                    }
                    if is_ponder {
                        if let Some(bm) = best_move {
                            info.ponder = Some(format!("{}{}", format_square(bm.from), format_square(bm.to)));
                        }
                    }
                    let _ = sender.send(info);
                }

                if let Some(pos) = root_moves.iter().position(|m| *m == mv) {
                    root_moves.swap(0, pos);
                }
            }
        }
    }

    best_move
}

/// Time-limited iterative deepening driver.
#[allow(dead_code)]
pub fn time_limited_search_with_sink(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_time: Duration,
    start_time: Instant,
    sink: Option<Arc<Mutex<Option<Move>>>>,
    info_sender: Option<Sender<uci_info::Info>>,
    is_ponder: bool,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut depth = 1u32;
    let mut last_depth_time = Duration::from_millis(1);

    const SAFETY_MARGIN: Duration = Duration::from_millis(5);
    const TIME_GROWTH_FACTOR: f32 = 2.0;

    while start_time.elapsed() + SAFETY_MARGIN < max_time {
        let elapsed = start_time.elapsed();
        let time_remaining = max_time.checked_sub(elapsed).unwrap_or_default();

        let estimated_next_time = last_depth_time.mul_f32(TIME_GROWTH_FACTOR);
        if estimated_next_time + SAFETY_MARGIN > time_remaining {
            break;
        }

        let depth_start = Instant::now();

        let mut legal_moves = Vec::new();
        board.generate_moves_into(&mut legal_moves);
        if legal_moves.is_empty() {
            return None;
        }
        if legal_moves.len() == 1 {
            return Some(legal_moves[0]);
        }

    legal_moves.sort_by_key(|m| -crate::board::mvv_lva_score(m, board));
    apply_tt_move_hint(&mut legal_moves[..], tt, board.hash);

    // Attempt aspiration window around previous depth's score
    const ASPIRATION_WINDOW: i32 = 50;
    let mut window = None;
    if last_depth_time != Duration::from_millis(1) {
        // We don't have score from previous depth here, but reuse last best_score if available
        // For simplicity, attempt no window unless caller has prev_score; keep None.
        window = None;
    }

    let (mut new_best_move, mut best_score, mut _completed) = board.run_root_search(
            tt,
            depth,
            &mut legal_moves,
            || start_time.elapsed() + SAFETY_MARGIN >= max_time,
            window,
        );

    // If aspiration window was used (none here) and failed, fallback (kept for parity with iterative version)

        if start_time.elapsed() + SAFETY_MARGIN < max_time {
            best_move = new_best_move;
            if let Some(ref s) = sink {
                let mut lock = match s.lock() {
                    Ok(g) => g,
                    Err(poisoned) => {
                        eprintln!("warning: sink mutex poisoned, recovering");
                        poisoned.into_inner()
                    }
                };
                *lock = best_move;
            }

            if let Some(ref sender) = info_sender {
                let nodes_total = crate::search_control::get_node_count();
                let elapsed_ms = start_time.elapsed().as_millis();
                let nps = if elapsed_ms > 0 {
                    Some(((nodes_total as u128 * 1000) / elapsed_ms) as u64)
                } else {
                    None
                };
                let pv = build_pv_from_tt(tt, board.hash);
                let mut info = uci_info::Info {
                    depth: Some(depth),
                    nodes: Some(nodes_total),
                    nps,
                    time_ms: Some(elapsed_ms),
                    score_cp: None,
                    score_mate: None,
                    pv: Some(pv),
                    seldepth: None,
                    ponder: None,
                };
                if best_score.abs() > (crate::constants::MATE_SCORE / 2) {
                    let mate_in = (crate::constants::MATE_SCORE - best_score.abs() + 1) / 2;
                    info.score_mate = Some(mate_in);
                } else {
                    info.score_cp = Some(best_score);
                }
                if is_ponder {
                    if let Some(bm) = best_move {
                        info.ponder = Some(format!("{}{}", format_square(bm.from), format_square(bm.to)));
                    }
                }
                let _ = sender.send(info);
            }

            last_depth_time = depth_start.elapsed();
            depth += 1;
        } else {
            break;
        }
    }

    best_move
}

/// Build a principal variation (PV) string by following TT best-move entries starting
/// from `start_hash`. Used to include a PV in UCI info messages.
#[allow(dead_code)]
pub fn build_pv_from_tt(tt: &TranspositionTable, start_hash: u64) -> String {
    let mut pv = Vec::new();
    if let Some(entry) = tt.probe(start_hash) {
        if let Some(mv) = entry.best_move {
            pv.push(mv);
        }
    }
    let pv_strs: Vec<String> = pv
        .iter()
        .map(|m| format!("{}{}", format_square(m.from), format_square(m.to)))
        .collect();
    pv_strs.join(" ")
}

/// If the transposition table contains a best-move hint for `hash`, and that move
/// is present in `moves`, swap it to index 0 so it will be searched first.
#[allow(dead_code)]
pub fn apply_tt_move_hint(moves: &mut [Move], tt: &TranspositionTable, hash: u64) {
    if let Some(entry) = tt.probe(hash) {
        if let Some(hm) = &entry.best_move {
            if let Some(pos) = moves.iter().position(|m| m == hm) {
                moves.swap(0, pos);
            }
        }
    }
}
