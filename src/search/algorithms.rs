use crate::transposition::transposition_table::TranspositionTable;
use crate::core::types::{Move, MoveList};
use crate::core::board::Board;
use crate::movegen::ordering::OrderingContext;
use crate::transposition::transposition_table::BoundType;
use crate::evaluation::eval;
use crate::core::constants;
use crate::movegen::ordering::order_moves;

/// Check if the current position is likely zugzwang (only king and pawns)
/// In such positions, null move pruning is unsafe
fn is_zugzwang_position(board: &Board) -> bool {
    let color_idx = if board.white_to_move { 0 } else { 1 };
    let opponent_idx = 1 - color_idx;
    
    // Count pieces for the side to move
    let mut piece_count = 0;
    for piece_type in 0..6 { // Pawn, Knight, Bishop, Rook, Queen, King
        piece_count += board.bitboards[color_idx][piece_type].count_ones();
    }
    
    // If we have very few pieces (king + pawns only), likely zugzwang
    // Also check if opponent has many pieces (we might need to move to defend)
    let opponent_piece_count = (0..6).map(|pt| board.bitboards[opponent_idx][pt].count_ones()).sum::<u32>();
    
    piece_count <= 4 && opponent_piece_count > 3
}

/// Negamax search with alpha-beta pruning
pub fn negamax(
    board: &mut Board,
    tt: &mut TranspositionTable,
    depth: u32,
    mut alpha: i32,
    mut beta: i32,
    moves_buf: &mut MoveList,
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

        // Internal Iterative Deepening (IID): if we have no TT move and depth is large,
        // do a shallow search (depth-2) to populate the TT with a good move for ordering.
        if hash_move.is_none() && depth >= 3 {
            // perform a reduced-depth search to get a usable TT hint
            let _ = negamax(board, tt, depth - 2, alpha, beta, moves_buf, ctx);
            if let Some(entry) = tt.probe(current_hash) {
                hash_move = entry.best_move;
            }
        }

        if crate::search::control::should_stop() {
            return 0;
        }

        if board.is_draw() {
            return 0;
        }

        crate::search::control::node_visited();

        if depth == 0 {
            // Take the child buffer for quiesce
            let mut child_buf = std::mem::take(&mut ctx.child_buf);
            child_buf.clear();
            let score = quiesce(board, alpha, beta, &mut child_buf, ctx);
            ctx.child_buf = child_buf;
            return score;
        }

        // Null-move pruning (Publius-style) with verification search on cutoff.
        // Only apply when depth is sufficiently large, not in check, not zugzwang, and not near mate
        if depth >= 3 && !board.is_in_check(board.current_color()) 
           && !is_zugzwang_position(board) 
           && beta.abs() < (constants::MATE_SCORE - 100) {
            
            // Adaptive reduction: deeper searches can afford larger reductions
            let r = if depth >= 6 { 3u32 } else { 2u32 };
            let null_info = board.make_null_move();
            let null_score = -negamax(board, tt, depth - 1 - r, -beta, -beta + 1, moves_buf, ctx);
            board.unmake_null_move(null_info);
            
            if null_score >= beta {
                // Verification search at the same reduced depth to confirm cutoff
                let verify_score = -negamax(board, tt, depth - 1 - r, -beta, -alpha, moves_buf, ctx);
                if verify_score >= beta {
                    tt.store(current_hash, depth, verify_score, BoundType::LowerBound, None);
                    return verify_score;
                }
                // Otherwise, fall through and search normally (no premature cutoff).
            }
        }

        moves_buf.clear();
        board.generate_moves_into(moves_buf);
        // Use ordering heuristics (TT move already extracted above)
        order_moves(ctx, board, &mut moves_buf[..], depth as usize, hash_move);

        if moves_buf.is_empty() {
            let current_color = board.current_color();
            return if board.is_in_check(current_color) {
                -(constants::MATE_SCORE - (100 - depth as i32))
            } else {
                0
            };
        }

        if let Some(hm) = &hash_move {
            if let Some(pos) = moves_buf.iter().position(|m| m == hm) {
                moves_buf.swap(0, pos);
            }
        }

        let mut best_score: i32 = -constants::MATE_SCORE * 2;
        let mut best_move_found: Option<Move> = None;

        // Take the reusable child buffer out of the ordering context so we can
        // borrow it independently without causing multiple mutable borrows of ctx.
        let mut child_buf = std::mem::take(&mut ctx.child_buf);
        child_buf.clear();
        child_buf.reserve(moves_buf.len().max(16));
        for (i, m) in moves_buf.iter().enumerate() {
            if crate::search::control::should_stop() {
                break;
            }
            // Capture attacker piece type before making the move (board state will change)
            let attacker_piece = board.piece_at(m.from).map(|(_c, p)| p);
            // Futility pruning: at very shallow depths (near leaf), skip quiet moves
            // that are unlikely to raise alpha. This is a conservative heuristic.
            let is_quiet = m.captured_piece.is_none() && m.promotion.is_none();
            if is_quiet && depth <= 2 {
                // small margin per ply (centipawns)
                let margin = if depth == 1 { 150 } else { 80 };
                let stand_pat = eval::evaluate(board);
                if stand_pat + margin <= alpha {
                    // treat as a non-improving move: continue to next move
                    continue;
                }
            }

            // Prepare to make the move and search. We'll apply Late Move Reductions (LMR)
            // for non-captures/non-promotions that appear late in the move ordering.
            let info = board.make_move(m);

            let mut score: i32;
            if i == 0 {
                // principal variation move - full depth
                score = -negamax(board, tt, depth - 1, -beta, -alpha, &mut child_buf, ctx);
            } else {
                let mut did_lmr = false;
                let mut reduced_score = -constants::MATE_SCORE * 2;

                // Determine if LMR is applicable: non-capture, no promotion, and depth >= 3
                if m.captured_piece.is_none() && m.promotion.is_none() && depth >= 3 {
                    // apply reductions only for sufficiently late moves
                    if i >= 4 {
                        // basic reduction formula: 1 + (i / 6), capped to depth-2
                        let mut red = 1u32 + (i as u32 / 6);
                        if red > depth.saturating_sub(2) {
                            red = depth.saturating_sub(2);
                        }
                        if red > 0 {
                            did_lmr = true;
                            let reduced_depth = depth - 1 - red;
                            reduced_score = -negamax(board, tt, reduced_depth, -alpha - 1, -alpha, &mut child_buf, ctx);
                        }
                    }
                }

                if did_lmr {
                    score = reduced_score;
                    // If reduced search suggests the move might be interesting, do full null-window then full-window re-search
                    if score > alpha && score < beta {
                        score = -negamax(board, tt, depth - 1, -alpha - 1, -alpha, &mut child_buf, ctx);
                        if score > alpha && score < beta {
                            score = -negamax(board, tt, depth - 1, -beta, -alpha, &mut child_buf, ctx);
                        }
                    }
                } else {
                    // No reduction: PVS-style search
                    score = -negamax(board, tt, depth - 1, -alpha - 1, -alpha, &mut child_buf, ctx);
                    if score > alpha && score < beta {
                        score = -negamax(board, tt, depth - 1, -beta, -alpha, &mut child_buf, ctx);
                    }
                }
            }

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

        // Return the child buffer to the ordering context before storing TT and returning
        ctx.child_buf = child_buf;

        tt.store(current_hash, depth, best_score, bound_type, best_move_found);

        best_score
    }

/// Quiescence search to avoid horizon effect
pub fn quiesce(
    board: &mut Board,
    mut alpha: i32,
    beta: i32,
    _moves_buf: &mut MoveList,  // Not used, kept for API compatibility
    ctx: &mut OrderingContext,
) -> i32 {
        let stand_pat_score = eval::evaluate(board);
        if board.is_draw() {
            return 0;
        }
        if crate::search::control::should_stop() {
            return stand_pat_score;
        }
        crate::search::control::node_visited();
        if stand_pat_score >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat_score);

        let mut local_buf = MoveList::new();
        board.generate_tactical_moves_into(&mut local_buf);
        // Optionally disable SEE-based pruning/ordering via env var for diagnostics.
        if std::env::var_os("CHESS_DISABLE_SEE").is_none() {
            // Use SEE to prune obviously losing captures and order by SEE value
            local_buf.retain(|m| {
                let s = crate::see::see_capture(board, m);
                s >= 0
            });
            local_buf.sort_by_key(|m| -crate::see::see_capture(board, m));
        }
        order_moves(ctx, board, &mut local_buf[..], 0, None);

        let mut best_score = stand_pat_score;
        let tactical_moves = local_buf.clone();
        for m in tactical_moves {
            if crate::search::control::should_stop() {
                break;
            }
            let info = board.make_move(&m);
            let score = -quiesce(board, -beta, -alpha, &mut MoveList::new(), ctx);
            board.unmake_move(&m, info);

            best_score = best_score.max(score);
            alpha = alpha.max(best_score);
            if alpha >= beta {
                break;
            }
        }

        alpha
    }