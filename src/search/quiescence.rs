use crate::core::types::MoveList;
use crate::core::board::Board;
use crate::evaluation::eval;
use crate::movegen::ordering::order_moves;
use crate::search::search_context::SearchContext;

/// Quiescence search to avoid horizon effect
pub fn quiesce(
    board: &mut Board,
    s_ctx: &mut SearchContext,
    mut alpha: i32,
    beta: i32,
) -> i32 {
    const MAX_PLY: usize = 32; // tightened for debugging
    crate::search::control::update_max_ply(s_ctx.ply);
    if cfg!(debug_assertions) {
        
    }
    if cfg!(debug_assertions) && s_ctx.ply > 20 {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("trace.txt")
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(f, "quiesce ply {}", s_ctx.ply)
            });
    }
    if s_ctx.ply >= MAX_PLY {
        if cfg!(debug_assertions) {
            
        }
        return eval::evaluate(board, s_ctx.pawn_hash_table);
    }
    let in_check = board.is_in_check(board.current_color());

    // In check: generate all moves to escape check
    if in_check {
        let mut all_moves = MoveList::new();
        board.generate_moves_into(&mut all_moves); // This generates all legal moves

        let mut best_score = -i32::MAX;
    for m in &all_moves {
        if crate::search::control::should_stop() {
            break;
        }
        let info = board.make_move(m);
        // Increment ply in child context to honor depth guards.
        let mut child_ctx = SearchContext {
            tt: s_ctx.tt,
            moves_buf: s_ctx.moves_buf,
            ordering_ctx: s_ctx.ordering_ctx,
            ply: s_ctx.ply + 1,
            pawn_hash_table: s_ctx.pawn_hash_table,
        };
        let score = -quiesce(board, &mut child_ctx, -beta, -alpha);
        board.unmake_move(m, info);

            best_score = best_score.max(score);
            alpha = alpha.max(best_score);
            if alpha >= beta {
                return beta; // Fail hard beta-cutoff
            }
        }
        return best_score; // Return the best score found to escape check
    }


    let static_eval = eval::evaluate(board, s_ctx.pawn_hash_table);
    // QS Futility Pruning
    if !in_check { // Only prune if not in check
        if static_eval + crate::core::config::search::QS_FUTILITY_MARGIN < alpha {
            return alpha;
        }
    }
    let mut stand_pat_score = static_eval;
    // Apply small statistical correction based on last move's destination.
    if let Some(last) = board.last_move_made {
        if let Some((_, piece)) = board.piece_at(last.to) {
            let corr = s_ctx
                .ordering_ctx
                .correction_for_square(piece, last.to.0 as u8);
            stand_pat_score += corr / 8;
        }
    }
    if board.is_draw() {
        return 0;
    }
    if crate::search::control::should_stop() {
        return stand_pat_score;
    }
    crate::search::control::node_visited();
    if crate::search::control::should_stop() {
        return stand_pat_score;
    }
    if stand_pat_score >= beta {
        return beta;
    }
    alpha = alpha.max(stand_pat_score);

    let mut local_buf = MoveList::new();
    board.generate_tactical_moves_into(&mut local_buf);

    // Apply SEE-based ordering (no pruning to keep tactical sacs)
    apply_see_ordering(board, &mut local_buf);
    order_moves(s_ctx.ordering_ctx, board, &mut local_buf[..], 0, None);

    let mut best_score = stand_pat_score;
    fn piece_value(p: crate::core::types::Piece) -> i32 {
        crate::core::config::evaluation::MATERIAL_MG[p as usize]
    }

    for m in &local_buf {
        if crate::search::control::should_stop() {
            break;
        }
        // QS SEE pruning: only consider captures that are not too bad
        if m.captured_piece.is_some() {
            let see_score = crate::movegen::see::see_capture(board, m);
            if see_score < crate::core::config::search::QS_SEE_PRUNING_MARGIN {
                continue;
            }
        }

        // Delta pruning: if even capturing the victim plus a margin can't beat alpha, skip.
        if let Some(victim) = m.captured_piece {
            let gain = piece_value(victim) + 50;
            if stand_pat_score + gain < alpha {
                continue;
            }
        }
        let info = board.make_move(m);
        let mut child_s_ctx = SearchContext {
            tt: s_ctx.tt,
            moves_buf: &mut MoveList::new(),
            ordering_ctx: s_ctx.ordering_ctx,
            ply: s_ctx.ply + 1,
            pawn_hash_table: s_ctx.pawn_hash_table,
        };
        let score = -quiesce(board, &mut child_s_ctx, -beta, -alpha);
        board.unmake_move(m, info);

        best_score = best_score.max(score);
        alpha = alpha.max(best_score);
        if alpha >= beta {
            break;
        }
    }

    alpha
}

/// Apply SEE-based ordering to tactical moves
fn apply_see_ordering(board: &Board, moves: &mut MoveList) {
    // Optionally disable SEE-based pruning/ordering via env var for diagnostics.
    if std::env::var_os("CHESS_DISABLE_SEE").is_none() {
        // Order by SEE value; keep all moves to preserve sacrificial tactics.
        moves.sort_by_key(|m| -crate::see::see_capture(board, m));
    }
}
