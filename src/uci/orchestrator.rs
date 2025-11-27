use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SearchLimits {
    pub depth: Option<u32>,
    pub movetime: Option<Duration>,
    pub wtime: Option<Duration>,
    pub btime: Option<Duration>,
    pub winc: Option<Duration>,
    pub binc: Option<Duration>,
    pub ponder: bool,
    pub nodes: Option<u64>,
}

use crate::core::board::Board;
use crate::engine::{SimpleEngine, SearchOptions, SearchEngine};
use crate::search::control as search_control;
use crate::transposition::transposition_table::TranspositionTable;
use crate::uci::protocol::{UciCommand, UciResponse};

/// Search orchestrator that manages search threads and state
pub struct SearchOrchestrator {
    board: Board,
    search_thread: Option<JoinHandle<()>>,
    search_best: Option<Arc<Mutex<Option<crate::core::types::Move>>>>,
    searching: bool,
    pondering: bool,
    info_sender: std::sync::mpsc::Sender<crate::uci::info::Info>,
}

impl SearchOrchestrator {
    pub fn new(info_sender: std::sync::mpsc::Sender<crate::uci::info::Info>) -> Self {
        Self {
            board: Board::new(),
            search_thread: None,
            search_best: None,
            searching: false,
            pondering: false,
            info_sender,
        }
    }

    /// Handle a UCI command and return appropriate responses
    pub fn handle_command(&mut self, command: &UciCommand) -> Vec<UciResponse> {
        match command {
            UciCommand::Uci => {
                vec![
                    UciResponse::IdName("MyRustEngine".to_string()),
                    UciResponse::IdAuthor("Dean Menezes".to_string()),
                    UciResponse::UciOk,
                ]
            }
            UciCommand::IsReady => {
                vec![UciResponse::ReadyOk]
            }
            UciCommand::UciNewGame => {
                self.board = Board::new();
                vec![]
            }
            UciCommand::Position { fen, moves } => {
                // Set position from FEN or startpos
                if let Some(fen_str) = fen {
                    match Board::try_from_fen(fen_str) {
                        Ok(board) => self.board = board,
                        Err(e) => {
                            eprintln!("Invalid FEN: {} - {:?}", fen_str, e);
                            return vec![];
                        }
                    }
                } else {
                    self.board = Board::new();
                }

                // Apply moves
                for move_str in moves {
                    let mut board_copy = self.board.clone();
                    if let Some(mv) = crate::uci::protocol::parse_uci_move(&mut board_copy, move_str) {
                        self.board.make_move(&mv);
                    } else {
                        eprintln!("Invalid move: {}", move_str);
                    }
                }
                vec![]
            }
            UciCommand::Go {
                depth,
                movetime,
                wtime,
                btime,
                winc,
                binc,
                infinite: _,
                ponder,
                nodes,
            } => {
                let limits = SearchLimits {
                    depth: *depth,
                    movetime: *movetime,
                    wtime: *wtime,
                    btime: *btime,
                    winc: *winc,
                    binc: *binc,
                    ponder: *ponder,
                    nodes: *nodes,
                };
                self.start_search(limits)
            }
            UciCommand::Stop => {
                self.stop_search()
            }
            UciCommand::PonderHit => {
                self.handle_ponder_hit()
            }
            UciCommand::Display => {
                println!("FEN: {}", self.board.to_fen());
                vec![]
            }
            UciCommand::Quit => {
                self.stop_search();
                vec![]
            }
        }
    }

    fn start_search(&mut self, limits: SearchLimits) -> Vec<UciResponse> {
        // Stop any existing search
        if self.searching {
            search_control::set_stop(true);
            if let Some(handle) = self.search_thread.take() {
                let _ = handle.join();
            }
            self.searching = false;
            self.pondering = false;
        }

        // Set pondering flag
        self.pondering = limits.ponder;

        // Set node limit if specified
        if let Some(n) = limits.nodes {
            search_control::set_node_limit(n);
        }

        // Compute movetime if not specified but time controls are
        let computed_movetime = limits.movetime.or_else(|| {
            self.compute_movetime(limits.wtime, limits.btime, limits.winc, limits.binc)
        });

        // Clone state for background search
        let board_clone = self.board.clone();
        let tt_clone = TranspositionTable::new(1024);
        let bm = Arc::new(Mutex::new(None::<crate::core::types::Move>));
        let bm_thread = bm.clone();
        self.search_best = Some(bm);
        search_control::reset();

        let info_tx = self.info_sender.clone();
        let is_ponder = limits.ponder;

        // Spawn search thread
        let handle = spawn(move || {
            let engine = SimpleEngine::new();
            let mut local_board = board_clone;
            let mut local_tt = tt_clone;

            let opts = Self::build_search_options(
                limits.depth,
                computed_movetime,
                is_ponder,
                Some(bm_thread),
                Some(info_tx),
            );

            search_control::set_stop(false);

            match engine.search(&mut local_board, &mut local_tt, opts) {
                Ok(res) => {
                    // Send bestmove response
                    if let Some(bm) = res.best_move {
                        println!("bestmove {}", crate::uci::protocol::format_uci_move(&bm));
                    } else {
                        println!("bestmove 0000");
                    }
                }
                Err(e) => {
                    eprintln!("search error: {}", e);
                    println!("bestmove 0000");
                }
            }
        });

        self.search_thread = Some(handle);
        self.searching = true;

        vec![] // No immediate response, search runs in background
    }

    fn stop_search(&mut self) -> Vec<UciResponse> {
        search_control::set_stop(true);
        if let Some(handle) = self.search_thread.take() {
            let _ = handle.join();
        }
        self.searching = false;

        // Return best move if available
        if let Some(bm_arc) = &self.search_best {
            let guard = match bm_arc.lock() {
                Ok(g) => g,
                Err(poisoned) => {
                    eprintln!("warning: bestmove mutex poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            if let Some(best) = *guard {
                vec![UciResponse::BestMove(Some(best))]
            } else {
                vec![UciResponse::BestMove(None)]
            }
        } else {
            vec![UciResponse::BestMove(None)]
        }
    }

    fn handle_ponder_hit(&mut self) -> Vec<UciResponse> {
        if self.pondering {
            search_control::set_stop(false);
            self.pondering = false;
        } else {
            search_control::set_stop(false);
        }
        vec![]
    }

    fn compute_movetime(
        &self,
        wtime: Option<Duration>,
        btime: Option<Duration>,
        winc: Option<Duration>,
        binc: Option<Duration>,
    ) -> Option<Duration> {
        let (time_left, inc) = if self.board.white_to_move {
            (wtime?, winc.unwrap_or(Duration::ZERO))
        } else {
            (btime?, binc.unwrap_or(Duration::ZERO))
        };

        // Simple time allocation: divide by 30 moves, subtract safety margin, add increment
        let moves_to_go = 30u64;
        let mut alloc_ms = time_left.as_millis() as u64 / moves_to_go;
        if alloc_ms > 50 {
            alloc_ms = alloc_ms.saturating_sub(50);
        }
        alloc_ms = alloc_ms.saturating_add(inc.as_millis() as u64 / 4);
        if alloc_ms == 0 {
            alloc_ms = 1;
        }

        Some(Duration::from_millis(alloc_ms))
    }

    fn build_search_options(
        depth: Option<u32>,
        movetime: Option<Duration>,
        is_ponder: bool,
        sink: Option<Arc<Mutex<Option<crate::core::types::Move>>>>,
        info_sender: Option<std::sync::mpsc::Sender<crate::uci::info::Info>>,
    ) -> SearchOptions {
        if let Some(d) = depth {
            SearchOptions {
                max_depth: Some(d),
                max_time: None,
                max_nodes: None,
                is_ponder,
                sink,
                info_sender,
                move_ordering: None,
            }
        } else if let Some(t) = movetime {
            SearchOptions {
                max_depth: None,
                max_time: Some(t),
                max_nodes: None,
                is_ponder,
                sink,
                info_sender,
                move_ordering: None,
            }
        } else {
            // Infinite or nodes-limited search
            SearchOptions {
                max_depth: Some(64),
                max_time: None,
                max_nodes: None,
                is_ponder,
                sink,
                info_sender,
                move_ordering: None,
            }
        }
    }
}
