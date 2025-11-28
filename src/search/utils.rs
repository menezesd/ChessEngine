use crate::transposition::transposition_table::TranspositionTable;
use crate::core::types::{Move, MoveList};
use crate::core::board::Board;
use crate::core::config::evaluation::*;
use crate::search::search_context::SearchContext;

/// Build principal variation string from transposition table by following best moves.
pub fn build_pv_from_tt(tt: &TranspositionTable, board: &Board) -> String {
    let mut pv_moves: Vec<Move> = Vec::new();
    let mut temp_board = board.clone();

    // Follow the TT best move chain, stopping on missing/illegal moves or repetition.
    for _ in 0..32 {
        let hash = temp_board.hash;
        let entry = match tt.probe(hash) {
            Some(e) => e,
            None => break,
        };
        let mv = match entry.best_move {
            Some(m) => m,
            None => break,
        };

        // Only follow legally generated moves to avoid stale TT entries.
        let mut moves = MoveList::new();
        temp_board.generate_moves_into(&mut moves);
        if !moves.iter().any(|m| *m == mv) {
            break;
        }

        let info = temp_board.make_move(&mv);
        pv_moves.push(mv);

        if temp_board.is_draw() {
            // Avoid cycling forever on repetitions/tablebase draws.
            temp_board.unmake_move(&mv, info);
            break;
        }
    }

    pv_moves
        .iter()
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
            return (best_move, best_score, false); // aborted mid-root, return last best
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
