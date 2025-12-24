mod log;
mod move_order;
mod params;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::tt::{BoundType, TranspositionTable};

use super::{format_square, MAX_PLY};
use super::{Board, Color, Move, MoveList, Piece, UnmakeInfo};
use log::{SearchInfo, SearchLogger, StdoutLogger};
pub use params::SearchParams;

pub const DEFAULT_TT_MB: usize = 1024;

pub struct SearchStats {
    pub nodes: u64,
    pub seldepth: u32,
    pub total_nodes: u64,
    pub max_nodes: u64,
}

const SELDEPTH_CAP: u32 = 64;

impl SearchStats {
    pub fn reset_search(&mut self) {
        self.nodes = 0;
        self.seldepth = 0;
        self.total_nodes = 0;
    }

    pub fn reset_iteration(&mut self) {
        self.nodes = 0;
        self.seldepth = 0;
    }
}

pub struct SearchTables {
    pub tt: TranspositionTable,
    pub killer_moves: [[Move; 2]; MAX_PLY],
    pub history: [i32; 4096],
    pub counter_moves: [[Move; 64]; 64],
}

pub struct SearchState {
    pub stats: SearchStats,
    pub tables: SearchTables,
    pub generation: u16,
    pub last_move: Move,
    pub hard_stop_at: Option<Instant>,
    pub params: SearchParams,
    pub trace: bool,
    pub logger: Box<dyn SearchLogger + Send>,
}

impl SearchState {
    pub fn new(tt_mb: usize) -> Self {
        SearchState {
            stats: SearchStats {
                nodes: 0,
                seldepth: 0,
                total_nodes: 0,
                max_nodes: 0,
            },
            tables: SearchTables {
                tt: TranspositionTable::new(tt_mb),
                killer_moves: [[super::EMPTY_MOVE; 2]; MAX_PLY],
                history: [0; 4096],
                counter_moves: [[super::EMPTY_MOVE; 64]; 64],
            },
            generation: 0,
            last_move: super::EMPTY_MOVE,
            hard_stop_at: None,
            params: SearchParams::default(),
            trace: false,
            logger: Box::new(StdoutLogger),
        }
    }

    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.stats.reset_search();
        self.last_move = super::EMPTY_MOVE;
        self.hard_stop_at = None;
    }

    pub fn set_max_nodes(&mut self, max_nodes: u64) {
        self.stats.max_nodes = max_nodes;
    }

    pub fn set_hard_stop_at(&mut self, stop_at: Option<Instant>) {
        self.hard_stop_at = stop_at;
    }

    pub fn params_mut(&mut self) -> &mut SearchParams {
        &mut self.params
    }

    pub fn params(&self) -> &SearchParams {
        &self.params
    }

    pub fn set_params(&mut self, params: SearchParams) {
        self.params = params;
    }

    pub fn trace(&self) -> bool {
        self.trace
    }

    pub fn set_trace(&mut self, trace: bool) {
        self.trace = trace;
    }

    pub fn set_logger(&mut self, logger: Box<dyn SearchLogger + Send>) {
        self.logger = logger;
    }

    #[allow(dead_code)]
    fn best_move_from_tt(&self, hash: u64) -> Option<Move> {
        self.tables.tt.probe(hash).and_then(|entry| entry.best_move)
    }

    pub fn reset_tables(&mut self, tt_mb: usize) {
        self.tables.tt = TranspositionTable::new(tt_mb);
        self.stats.reset_search();
    }

    pub fn hashfull_per_mille(&self) -> u32 {
        self.tables.tt.hashfull_per_mille()
    }

    fn record_killer(&mut self, ply: usize, mv: Move) {
        if ply >= MAX_PLY {
            return;
        }
        if self.tables.killer_moves[ply][0] != mv {
            self.tables.killer_moves[ply][1] = self.tables.killer_moves[ply][0];
            self.tables.killer_moves[ply][0] = mv;
        }
    }

    fn is_killer(&self, ply: usize, mv: Move) -> bool {
        if ply >= MAX_PLY {
            return false;
        }
        self.tables.killer_moves[ply][0] == mv || self.tables.killer_moves[ply][1] == mv
    }

    fn add_history(&mut self, mv: Move, depth: u32) {
        let idx = move_history_index(mv);
        if idx < self.tables.history.len() {
            self.tables.history[idx] =
                self.tables.history[idx].saturating_add((depth * depth) as i32);
        }
    }

    fn history_score(&self, mv: Move) -> i32 {
        let idx = move_history_index(mv);
        if idx < self.tables.history.len() {
            self.tables.history[idx]
        } else {
            0
        }
    }

    fn set_counter_move(&mut self, prev: Move, reply: Move) {
        let from = prev.from.index().0 as usize;
        let to = prev.to.index().0 as usize;
        self.tables.counter_moves[from][to] = reply;
    }

    fn get_counter_move(&self, prev: Move) -> Option<Move> {
        let from = prev.from.index().0 as usize;
        let to = prev.to.index().0 as usize;
        let mv = self.tables.counter_moves[from][to];
        if mv == super::EMPTY_MOVE {
            None
        } else {
            Some(mv)
        }
    }

    fn should_stop(&mut self, stop: &AtomicBool) -> bool {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        if let Some(deadline) = self.hard_stop_at {
            if self.stats.nodes & 2047 == 0 && Instant::now() >= deadline {
                stop.store(true, Ordering::Relaxed);
                return true;
            }
        }
        false
    }
}

impl Default for SearchState {
    fn default() -> Self {
        SearchState::new(DEFAULT_TT_MB)
    }
}

fn move_history_index(m: Move) -> usize {
    let from = m.from.index().0 as usize;
    let to = m.to.index().0 as usize;
    from * 64 + to
}

pub struct SearchLimits {
    pub clock: std::sync::Arc<SearchClock>,
    pub stop: std::sync::Arc<AtomicBool>,
}

pub struct SearchClock {
    start_time: Mutex<Instant>,
    soft_deadline: Mutex<Option<Instant>>,
    hard_deadline: Mutex<Option<Instant>>,
}

impl SearchClock {
    pub fn new(
        start_time: Instant,
        soft_deadline: Option<Instant>,
        hard_deadline: Option<Instant>,
    ) -> Self {
        SearchClock {
            start_time: Mutex::new(start_time),
            soft_deadline: Mutex::new(soft_deadline),
            hard_deadline: Mutex::new(hard_deadline),
        }
    }

    pub fn reset(
        &self,
        start_time: Instant,
        soft_deadline: Option<Instant>,
        hard_deadline: Option<Instant>,
    ) {
        if let Ok(mut start) = self.start_time.lock() {
            *start = start_time;
        }
        if let Ok(mut soft) = self.soft_deadline.lock() {
            *soft = soft_deadline;
        }
        if let Ok(mut hard) = self.hard_deadline.lock() {
            *hard = hard_deadline;
        }
    }

    pub fn snapshot(&self) -> (Instant, Option<Instant>, Option<Instant>) {
        let start_time = *self.start_time.lock().unwrap();
        let soft_deadline = *self.soft_deadline.lock().unwrap();
        let hard_deadline = *self.hard_deadline.lock().unwrap();
        (start_time, soft_deadline, hard_deadline)
    }
}

pub struct SearchContext<'a> {
    pub board: &'a mut Board,
    pub state: &'a mut SearchState,
    pub stop: &'a AtomicBool,
    pub start_time: Instant,
}

impl<'a> SearchContext<'a> {
    fn nps(&self) -> u64 {
        let elapsed_ms = self.start_time.elapsed().as_millis().max(1) as u64;
        (self.state.stats.nodes * 1000) / elapsed_ms
    }
}

pub enum TerminalState {
    Checkmate,
    Stalemate,
    Draw,
}

pub struct Position<'a> {
    board: &'a mut Board,
}

impl<'a> Position<'a> {
    pub fn new(board: &'a mut Board) -> Self {
        Position { board }
    }

    pub fn side_to_move(&self) -> Color {
        if self.board.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    pub fn legal_moves(&mut self) -> MoveList {
        self.board.generate_moves()
    }

    pub fn terminal_state(&mut self) -> Option<TerminalState> {
        if self.board.is_draw() {
            return Some(TerminalState::Draw);
        }
        let in_check = self.board.is_in_check(self.board.current_color());
        let legal_moves = self.board.generate_moves();
        if legal_moves.is_empty() {
            return Some(if in_check {
                TerminalState::Checkmate
            } else {
                TerminalState::Stalemate
            });
        }
        None
    }

    pub fn terminal_score(&self, in_check: bool, depth: u32) -> i32 {
        if in_check {
            -(MATE_SCORE - (100 - depth as i32))
        } else {
            0
        }
    }
}

const KING_VALUE: i32 = 20000;
const MATE_SCORE: i32 = KING_VALUE * 10;

impl Board {
    fn draw_aware(&self, eval: i32) -> bool {
        eval <= -150
    }

    fn null_move_safe_endgame(&self) -> bool {
        let phase_table = [0i32, 1, 1, 2, 4, 0];
        let mut phase_units = 0i32;
        for color in 0..2 {
            for (piece_idx, phase_value) in phase_table.iter().enumerate() {
                let count = self.pieces[color][piece_idx].0.count_ones() as i32;
                phase_units += count * *phase_value;
            }
        }
        phase_units > 6
    }

    #[allow(clippy::too_many_arguments)]
    fn try_null_move(
        &mut self,
        state: &mut SearchState,
        params: &SearchParams,
        depth: u32,
        ply: u32,
        stop: &AtomicBool,
        beta: i32,
        in_check: bool,
    ) -> Option<i32> {
        if in_check || depth < params.null_min_depth || self.is_theoretical_draw() {
            return None;
        }
        if !self.null_move_safe_endgame() {
            return None;
        }

        let null_info = self.make_null_move();
        let reduced_depth = depth.saturating_sub(1 + params.null_reduction);
        let score = -self.negamax(state, reduced_depth, ply + 1, stop, -beta, -beta + 1);
        self.unmake_null_move(null_info);

        if score >= beta {
            if depth >= params.null_verification_depth {
                let verify = self.negamax(
                    state,
                    depth.saturating_sub(1),
                    ply + 1,
                    stop,
                    beta - 1,
                    beta,
                );
                if verify >= beta {
                    return Some(score);
                }
            } else {
                return Some(score);
            }
        }
        None
    }

    pub(crate) fn negamax(
        &mut self,
        state: &mut SearchState,
        depth: u32,
        ply: u32,
        stop: &AtomicBool,
        mut alpha: i32,
        mut beta: i32,
    ) -> i32 {
        if state.should_stop(stop) {
            return 0;
        }
        let params = state.params;
        if ply as usize >= MAX_PLY - 1 {
            return self.eval_for_side();
        }
        state.stats.nodes += 1;
        state.stats.total_nodes += 1;
        if state.stats.max_nodes > 0 && state.stats.total_nodes >= state.stats.max_nodes {
            stop.store(true, Ordering::Relaxed);
            return 0;
        }
        if ply > state.stats.seldepth && ply <= SELDEPTH_CAP {
            state.stats.seldepth = ply;
        }
        if self.is_draw() {
            return 0;
        }

        let original_alpha = alpha;
        let current_hash = self.hash;

        let mut hash_move: Option<Move> = None;
        let mut tt_eval: Option<i32> = None;
        if let Some(entry) = state.tables.tt.probe(current_hash) {
            if entry.depth >= depth {
                let score = adjust_mate_score_for_retrieve(entry.score, ply);
                match entry.bound_type {
                    BoundType::Exact => return score,
                    BoundType::LowerBound => alpha = alpha.max(score),
                    BoundType::UpperBound => beta = beta.min(score),
                }
                if alpha >= beta {
                    return score;
                }
            }
            hash_move = entry.best_move;
            tt_eval = Some(entry.eval);
        }

        let in_check = self.is_in_check(self.current_color());

        if depth == 0 {
            return self.quiesce(state, ply, stop, alpha, beta);
        }

        let mut depth = depth;
        if depth >= params.iir_min_depth {
            depth = depth.saturating_sub(1);
        }
        if in_check {
            depth = depth.saturating_add(1);
        }

        let eval_at_node = if in_check { 0 } else { self.eval_for_side() };
        let draw_aware = !in_check && self.draw_aware(eval_at_node);

        if !draw_aware {
            if let Some(score) =
                self.try_null_move(state, &params, depth, ply, stop, beta, in_check)
            {
                return score;
            }
        }

        if !in_check && !draw_aware && depth <= 2 {
            let stand_pat = eval_at_node;
            if stand_pat + params.razor_margin < alpha {
                return self.quiesce(state, ply, stop, alpha, beta);
            }
        }

        let mut legal_moves = self.generate_moves();
        let counter_move = state.get_counter_move(state.last_move);
        move_order::order_moves(self, state, &mut legal_moves, ply, hash_move, counter_move);

        if legal_moves.is_empty() {
            let position = Position::new(self);
            return position.terminal_score(in_check, depth);
        }

        if let Some(hm) = &hash_move {
            if let Some(pos) = legal_moves.as_slice().iter().position(|m| m == hm) {
                legal_moves.as_mut_slice().swap(0, pos);
            }
        }

        let mut best_score = -MATE_SCORE * 2;
        let mut best_move_found: Option<Move> = None;
        let singular_target = hash_move.map(|hm| (hm, alpha + params.singular_margin));

        let stand_pat = if in_check {
            alpha
        } else {
            tt_eval.unwrap_or(eval_at_node)
        };

        if !in_check
            && !draw_aware
            && depth <= 4
            && stand_pat - params.rfp_margin * depth as i32 >= beta
        {
            return stand_pat;
        }

        if !in_check
            && !draw_aware
            && depth <= 3
            && stand_pat + params.static_null_margin * depth as i32 <= alpha
        {
            return stand_pat;
        }

        for (i, m) in legal_moves.iter().enumerate() {
            if state.should_stop(stop) {
                break;
            }
            if !draw_aware
                && depth >= params.lmp_min_depth
                && i >= params.lmp_move_limit
                && m.captured_piece.is_none()
                && m.promotion.is_none()
                && !in_check
            {
                break;
            }
            if !in_check
                && !draw_aware
                && depth <= 2
                && m.captured_piece.is_none()
                && m.promotion.is_none()
                && stand_pat + params.futility_margin <= alpha
            {
                continue;
            }
            let info = self.make_move(m);
            let prev_last = state.last_move;
            state.last_move = *m;
            let mut new_depth = depth - 1;
            if !in_check
                && depth >= params.lmr_min_depth
                && i >= params.lmr_min_move
                && m.captured_piece.is_none()
                && m.promotion.is_none()
            {
                new_depth = new_depth.saturating_sub(params.lmr_reduction);
            }

            let score = if i == 0 {
                if let Some((hm, target)) = singular_target {
                    if *m == hm && depth >= 6 {
                        let mut reduced = new_depth.saturating_sub(2);
                        let sing_score =
                            -self.negamax(state, reduced, ply + 1, stop, -target, -alpha);
                        if sing_score > -target {
                            reduced = new_depth;
                        }
                        -self.negamax(state, reduced, ply + 1, stop, -beta, -alpha)
                    } else {
                        -self.negamax(state, new_depth, ply + 1, stop, -beta, -alpha)
                    }
                } else {
                    -self.negamax(state, new_depth, ply + 1, stop, -beta, -alpha)
                }
            } else {
                let mut score = -self.negamax(state, new_depth, ply + 1, stop, -alpha - 1, -alpha);
                if score > alpha && score < beta {
                    score = -self.negamax(state, new_depth, ply + 1, stop, -beta, -alpha);
                }
                score
            };
            state.last_move = prev_last;
            self.unmake_move(m, info);

            if score > best_score {
                best_score = score;
                best_move_found = Some(*m);
            }

            alpha = alpha.max(best_score);

            if alpha >= beta {
                let is_quiet =
                    m.captured_piece.is_none() && m.promotion.is_none() && !m.is_en_passant;
                if is_quiet {
                    state.record_killer(ply as usize, *m);
                    state.add_history(*m, depth);
                    state.set_counter_move(state.last_move, *m);
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

        state.tables.tt.store(
            current_hash,
            depth,
            adjust_mate_score_for_store(best_score, ply),
            bound_type,
            best_move_found,
            state.generation,
            eval_at_node,
        );

        best_score
    }

    fn quiesce(
        &mut self,
        state: &mut SearchState,
        ply: u32,
        stop: &AtomicBool,
        mut alpha: i32,
        beta: i32,
    ) -> i32 {
        if state.should_stop(stop) {
            return 0;
        }
        let params = state.params;
        if ply as usize >= MAX_PLY - 1 {
            return self.eval_for_side();
        }
        state.stats.nodes += 1;
        state.stats.total_nodes += 1;
        if state.stats.max_nodes > 0 && state.stats.total_nodes >= state.stats.max_nodes {
            stop.store(true, Ordering::Relaxed);
            return 0;
        }
        if ply > state.stats.seldepth && ply <= SELDEPTH_CAP {
            state.stats.seldepth = ply;
        }
        if self.is_draw() {
            return 0;
        }

        let in_check = self.is_in_check(self.current_color());
        if in_check {
            let mut legal_moves = self.generate_moves();
            if legal_moves.is_empty() {
                let position = Position::new(self);
                return position.terminal_score(true, 0);
            }
            legal_moves
                .as_mut_slice()
                .sort_by_key(|m| -move_order::mvv_lva_score(m, self));

            let mut best_score = -MATE_SCORE * 2;
            for m in legal_moves.iter() {
                if state.should_stop(stop) {
                    break;
                }
                let info = self.make_move(m);
                let score = -self.quiesce(state, ply + 1, stop, -beta, -alpha);
                self.unmake_move(m, info);

                best_score = best_score.max(score);
                alpha = alpha.max(best_score);
                if alpha >= beta {
                    break;
                }
            }
            return alpha;
        }

        let stand_pat_score = self.eval_for_side();

        if stand_pat_score >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat_score);

        let mut tactical_moves = self.generate_tactical_moves();
        tactical_moves
            .as_mut_slice()
            .sort_by_key(|m| -move_order::mvv_lva_score(m, self));

        let mut best_score = stand_pat_score;

        for m in tactical_moves.iter() {
            if state.should_stop(stop) {
                break;
            }
            let bad_capture = move_order::is_bad_capture(self, m);
            let capture_value = m.captured_piece.map(move_order::piece_value).unwrap_or(0);
            let mut extra_margin = 0;
            if let Some(piece) = m.captured_piece {
                extra_margin += match piece {
                    Piece::Pawn => 0,
                    Piece::Knight | Piece::Bishop => 50,
                    Piece::Rook => 75,
                    Piece::Queen => 100,
                    _ => 0,
                };
            }
            if m.promotion.is_some() {
                extra_margin += 300;
            }
            let info = self.make_move(m);
            let gives_check = self.is_in_check(self.current_color());
            if !gives_check
                && stand_pat_score + capture_value + params.delta_margin + extra_margin < alpha
                && !m.is_en_passant
            {
                self.unmake_move(m, info);
                continue;
            }
            if bad_capture && !gives_check {
                self.unmake_move(m, info);
                continue;
            }
            let score = -self.quiesce(state, ply + 1, stop, -beta, -alpha);
            self.unmake_move(m, info);

            best_score = best_score.max(score);
            alpha = alpha.max(best_score);

            if alpha >= beta {
                break;
            }
        }

        alpha
    }
}

fn format_uci_move_for_info(mv: &Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => '?',
        });
    }
    s
}

fn format_score_for_info(score: i32) -> String {
    if score.abs() >= MATE_SCORE - 100 {
        let mate_plies = (MATE_SCORE - score.abs()).max(0) as u32;
        let mate_moves = mate_plies.div_ceil(2);
        let signed = if score > 0 {
            mate_moves as i32
        } else {
            -(mate_moves as i32)
        };
        format!("mate {}", signed)
    } else {
        format!("cp {}", score)
    }
}

fn emit_info(ctx: &mut SearchContext, depth: u32, score: i32, best_move: Move) {
    let pv_moves = if depth <= 2 || depth.is_multiple_of(2) {
        build_pv(ctx.board, ctx.state, 10)
    } else {
        Vec::new()
    };
    let pv = if pv_moves.is_empty() {
        format_uci_move_for_info(&best_move)
    } else {
        format_pv(&pv_moves)
    };
    let info = SearchInfo {
        depth,
        seldepth: ctx.state.stats.seldepth,
        score: format_score_for_info(score),
        nodes: ctx.state.stats.nodes,
        nps: ctx.nps(),
        hashfull: ctx.state.hashfull_per_mille(),
        time_ms: ctx.start_time.elapsed().as_millis().max(1),
        pv,
    };
    ctx.state.logger.info(&info);
}

fn emit_root_move(ctx: &mut SearchContext, depth: u32, score: i32, mv: Move) {
    let info = SearchInfo {
        depth,
        seldepth: ctx.state.stats.seldepth,
        score: format_score_for_info(score),
        nodes: ctx.state.stats.nodes,
        nps: ctx.nps(),
        hashfull: ctx.state.hashfull_per_mille(),
        time_ms: ctx.start_time.elapsed().as_millis().max(1),
        pv: format_uci_move_for_info(&mv),
    };
    ctx.state.logger.info(&info);
}

fn format_pv(moves: &[Move]) -> String {
    moves
        .iter()
        .map(format_uci_move_for_info)
        .collect::<Vec<String>>()
        .join(" ")
}

fn build_pv(board: &mut Board, state: &SearchState, max_len: usize) -> Vec<Move> {
    let mut pv = Vec::new();
    let mut history: Vec<(Move, UnmakeInfo)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    pv.reserve(max_len);
    history.reserve(max_len);

    for _ in 0..max_len {
        let hash = board.hash();
        if !seen.insert(hash) {
            break;
        }
        let entry = match state.tables.tt.probe(hash) {
            Some(e) => e,
            None => break,
        };
        let mv = match entry.best_move {
            Some(m) => m,
            None => break,
        };
        let legal_moves = board.generate_moves();
        if !legal_moves.as_slice().contains(&mv) {
            break;
        }
        let info = board.make_move(&mv);
        history.push((mv, info));
        pv.push(mv);
    }

    while let Some((mv, info)) = history.pop() {
        board.unmake_move(&mv, info);
    }

    pv
}

pub fn find_best_move(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    stop: &AtomicBool,
) -> Option<Move> {
    state.set_hard_stop_at(None);
    let mut best_move: Option<Move> = None;
    let mut last_score = 0;
    let start_time = Instant::now();
    let mut ctx = SearchContext {
        board,
        state,
        stop,
        start_time,
    };

    let legal_moves = ctx.board.generate_moves();
    if legal_moves.is_empty() {
        return None;
    }
    if legal_moves.len() == 1 {
        return legal_moves.first();
    }
    let mut root_moves = legal_moves.clone();

    for depth in 1..=max_depth {
        if ctx.state.should_stop(ctx.stop) {
            break;
        }
        let window = if depth <= 2 { MATE_SCORE } else { 50 };
        let mut alpha = last_score - window;
        let mut beta = last_score + window;

        let (mut current_best_score, mut current_best_move) = root_search(
            ctx.board,
            ctx.state,
            depth,
            alpha,
            beta,
            ctx.stop,
            &mut root_moves,
            best_move,
        );

        if current_best_score <= alpha || current_best_score >= beta {
            alpha = -MATE_SCORE * 2;
            beta = MATE_SCORE * 2;
            let result = root_search(
                ctx.board,
                ctx.state,
                depth,
                alpha,
                beta,
                ctx.stop,
                &mut root_moves,
                best_move,
            );
            current_best_score = result.0;
            current_best_move = result.1;
        }

        if let Some(mv) = current_best_move {
            best_move = Some(mv);
            last_score = current_best_score;

            if let Some(pos) = root_moves.as_slice().iter().position(|m| *m == mv) {
                root_moves.as_mut_slice().swap(0, pos);
            }
            if ctx.state.trace() {
                emit_info(&mut ctx, depth, current_best_score, mv);
            }
        }
    }

    best_move
}

pub fn find_best_move_with_time(
    board: &mut Board,
    state: &mut SearchState,
    limits: SearchLimits,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut depth = 1;
    let mut last_depth_time = Duration::from_millis(1);
    let mut last_score = 0;

    const SAFETY_MARGIN: Duration = Duration::from_millis(5);
    const TIME_GROWTH_FACTOR: f32 = 2.0;

    while !limits.stop.load(Ordering::Relaxed) {
        let (start_time, soft_deadline, hard_deadline) = limits.clock.snapshot();
        let now = Instant::now();
        let infinite = hard_deadline.is_none();
        let mut ctx = SearchContext {
            board,
            state,
            stop: &limits.stop,
            start_time,
        };
        let stop_at = match (soft_deadline, hard_deadline) {
            (Some(soft), Some(hard)) => Some(soft.min(hard)),
            (Some(soft), None) => Some(soft),
            (None, Some(hard)) => Some(hard),
            (None, None) => None,
        };
        ctx.state.set_hard_stop_at(stop_at);
        if let Some(hard_deadline) = hard_deadline {
            if now + SAFETY_MARGIN >= hard_deadline {
                break;
            }
        }
        let time_remaining = if let Some(soft_deadline) = soft_deadline {
            soft_deadline.saturating_duration_since(now)
        } else {
            Duration::from_secs(u64::MAX / 2)
        };

        if !infinite {
            let estimated_next_time = last_depth_time.mul_f32(TIME_GROWTH_FACTOR);
            if estimated_next_time + SAFETY_MARGIN > time_remaining {
                break;
            }
        }

        let depth_start = Instant::now();
        ctx.state.stats.reset_iteration();

        let mut legal_moves = ctx.board.generate_moves();

        if legal_moves.is_empty() {
            return None;
        }

        if legal_moves.len() == 1 {
            return legal_moves.first();
        }

        let window = if depth <= 2 { MATE_SCORE } else { 50 };
        let mut alpha = last_score - window;
        let mut beta = last_score + window;

        let (mut best_score, mut new_best_move) = root_search(
            ctx.board,
            ctx.state,
            depth,
            alpha,
            beta,
            &limits.stop,
            &mut legal_moves,
            best_move,
        );

        if best_score <= alpha || best_score >= beta {
            alpha = -MATE_SCORE * 2;
            beta = MATE_SCORE * 2;
            let result = root_search(
                ctx.board,
                ctx.state,
                depth,
                alpha,
                beta,
                &limits.stop,
                &mut legal_moves,
                best_move,
            );
            best_score = result.0;
            new_best_move = result.1;
        }

        if !limits.stop.load(Ordering::Relaxed) {
            best_move = new_best_move;
            last_depth_time = depth_start.elapsed();
            last_score = best_score;
            if let Some(mv) = best_move {
                emit_info(&mut ctx, depth, best_score, mv);
                if let Some(soft_deadline) = soft_deadline {
                    let now = Instant::now();
                    if now + SAFETY_MARGIN >= soft_deadline {
                        break;
                    }
                }
            }
            depth += 1;
        }
    }

    if best_move.is_none() {
        let legal_moves = board.generate_moves();
        if !legal_moves.is_empty() {
            return legal_moves.first();
        }
    }

    best_move
}

#[allow(clippy::too_many_arguments)]
fn root_search(
    board: &mut Board,
    state: &mut SearchState,
    depth: u32,
    mut alpha: i32,
    beta: i32,
    stop: &AtomicBool,
    root_moves: &mut MoveList,
    pv_move: Option<Move>,
) -> (i32, Option<Move>) {
    if stop.load(Ordering::Relaxed) {
        return (0, None);
    }

    let hash_move = state
        .tables
        .tt
        .probe(board.hash())
        .and_then(|e| e.best_move);
    move_order::order_root_moves(board, state, root_moves, hash_move, pv_move);
    if let Some(entry) = state.tables.tt.probe(board.hash()) {
        if let Some(hm) = &entry.best_move {
            if let Some(pos) = root_moves.as_slice().iter().position(|m| m == hm) {
                root_moves.as_mut_slice().swap(0, pos);
            }
        }
    }

    let mut best_score = -MATE_SCORE * 2;
    let mut best_move = if root_moves.is_empty() {
        None
    } else {
        root_moves.first()
    };

    let mut ctx = SearchContext {
        board,
        state,
        stop,
        start_time: Instant::now(),
    };

    for m in root_moves.iter() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let info = ctx.board.make_move(m);
        let score = -ctx
            .board
            .negamax(ctx.state, depth - 1, 1, ctx.stop, -beta, -alpha);
        ctx.board.unmake_move(m, info);

        if ctx.state.trace() {
            emit_root_move(&mut ctx, depth, score, *m);
        }

        if score > best_score {
            best_score = score;
            best_move = Some(*m);
        }

        alpha = alpha.max(best_score);
        if alpha >= beta {
            break;
        }
    }

    (best_score, best_move)
}

fn adjust_mate_score_for_store(score: i32, ply: u32) -> i32 {
    if score.abs() >= MATE_SCORE - 100 {
        if score > 0 {
            score + ply as i32
        } else {
            score - ply as i32
        }
    } else {
        score
    }
}

fn adjust_mate_score_for_retrieve(score: i32, ply: u32) -> i32 {
    if score.abs() >= MATE_SCORE - 100 {
        if score > 0 {
            score - ply as i32
        } else {
            score + ply as i32
        }
    } else {
        score
    }
}
