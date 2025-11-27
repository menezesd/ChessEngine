use crate::transposition::transposition_table::TranspositionTable;
use crate::core::types::Move;
use crate::core::board::Board;
use crate::core::config::evaluation::*;
use crate::search::search_context::SearchContext;

/// Build principal variation string from transposition table
#[allow(clippy::never_loop)]
pub fn build_pv_from_tt(tt: &TranspositionTable, start_hash: u64) -> String {
        let mut pv = Vec::new();
        let current_hash = start_hash;

        while let Some(entry) = tt.probe(current_hash) {
            if let Some(mv) = entry.best_move {
                pv.push(mv);
                // For simplicity, just break after first move to avoid complex board state management
                break;
            } else {
                break;
            }
        }

        pv.iter()
            .map(|m| format!("{}{}", crate::core::types::format_square(m.from), crate::core::types::format_square(m.to)))
            .collect::<Vec<_>>()
            .join(" ")
    }

/// Apply transposition table move hint to move ordering
pub fn apply_tt_move_hint(moves: &mut [Move], tt: &TranspositionTable, hash: u64) {
        if let Some(entry) = tt.probe(hash) {
            if let Some(hm) = &entry.best_move {
                if let Some(pos) = moves.iter().position(|m| m == hm) {
                    moves.swap(0, pos);
                }
            }
        }
    }

/// Run a single root depth search over root_moves
pub fn run_root_search<F>(
    board: &mut Board,
    s_ctx: &mut SearchContext,
    depth: u32,
    root_moves: &mut [Move],
    mut should_abort: F,
    window: Option<(i32, i32)>,
) -> (Option<Move>, i32, bool)
    where
        F: FnMut() -> bool,
    {
        let mut best_move: Option<Move> = None;

        // Alpha/Beta window for root search
        let mut alpha = -MATE_SCORE * 2;
        let beta = MATE_SCORE * 2;
        let mut best_score = -MATE_SCORE * 2;

        // Allow TT to promote a suggested move to the front for ordering
        apply_tt_move_hint(&mut root_moves[..], s_ctx.tt, board.hash);

        for (idx, m) in root_moves.iter().enumerate() {
            if should_abort() {
                return (None, 0, false); // aborted mid-root
            }
            let info = board.make_move(m);
            // Use the aspiration window for the first root move if provided
            let score = if idx == 0 {
                if let Some((w_alpha, w_beta)) = window {
                    -crate::search::algorithms::negamax(board, s_ctx, depth - 1, w_alpha, w_beta)
                } else {
                    -crate::search::algorithms::negamax(board, s_ctx, depth - 1, -beta, -alpha)
                }
            } else {
                -crate::search::algorithms::negamax(board, s_ctx, depth - 1, -beta, -alpha)
            };
            board.unmake_move(m, info);

            if score > best_score {
                best_score = score;
                best_move = Some(*m);
            }

            alpha = alpha.max(best_score);
        }

        (best_move, best_score, true)
    }