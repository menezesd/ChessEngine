mod go;
mod output;
mod state;

use std::io::{self, BufRead, Write};
use std::time::Instant;

use crate::board::DEFAULT_TT_MB;
use crate::engine::EngineController;
use crate::eval_math::{blended_eval, scaled_eval};
use crate::uci::command::{parse_uci_command, UciCommand};
use crate::uci::options::{UciOptionAction, UciOptions};
use crate::uci::parse_position_command;
use crate::uci::print::print_perft_info;
use crate::uci::report::print_ready;

use output::default_info_callback;
use state::UciState;

const KNOWN_COMMANDS: &[&str] = &[
    "uci",
    "isready",
    "ucinewgame",
    "position",
    "go",
    "eval",
    "evalfeatures",
    "perft",
    "setoption",
    "debug",
    "stop",
    "ponderhit",
    "quit",
];

pub(super) struct UciSession {
    pub(in crate::uci::session) engine: EngineController,
    pub(in crate::uci::session) options: UciOptions,
    pub(in crate::uci::session) state: UciState,
}

impl UciSession {
    fn new(default_tt_mb: usize) -> Self {
        let options = UciOptions::new(default_tt_mb);
        let mut engine = EngineController::new(options.hash_mb);
        engine.set_info_callback(Some(default_info_callback()));
        UciSession {
            engine,
            options,
            state: UciState::default(),
        }
    }

    /// Handle the "setoption" command
    fn handle_setoption(&mut self, name: &str, value: Option<&str>) {
        self.engine.stop_search();
        let action = self
            .engine
            .with_search_state(|state| self.options.apply_setoption(name, value, state))
            .flatten();
        if let Some(action) = action {
            self.apply_option_action(action);
        }
    }

    fn apply_option_action(&mut self, action: UciOptionAction) {
        match action {
            UciOptionAction::ReinitHash(new_mb) => {
                self.engine.resize_hash(new_mb);
            }
            UciOptionAction::SetThreads(threads) => {
                self.engine.set_threads(threads);
            }
        }
    }

    fn handle_position(&mut self, line: &str) {
        self.engine.stop_search();
        let parts: Vec<&str> = line.split_whitespace().collect();
        parse_position_command(self.engine.board_mut(), &parts);
    }

    fn handle_perft(&mut self, depth: usize) {
        self.engine.stop_search();
        let start = Instant::now();
        let nodes = self.engine.board_mut().perft(depth);
        print_perft_info(depth, nodes, start.elapsed());
    }

    fn handle_eval(&self) {
        self.engine.with_search_state_ref(|state| {
            let nnue = state.tables.nnue.as_ref().map(|network| {
                scaled_eval(
                    self.engine.board().evaluate_nnue(network),
                    state.nnue_eval_scale,
                )
            });
            let blend = state.nnue_hce_blend;
            let needs_hce = nnue.is_none() || blend < 100 || !state.static_eval_options.nnue_pure;
            let hce = needs_hce.then(|| {
                if state.hce_options.use_full {
                    if state.hce_options.use_tuned {
                        self.engine.board().evaluate_tuned_hce()
                    } else {
                        self.engine.board().evaluate()
                    }
                } else {
                    self.engine.board().evaluate_simple()
                }
            });
            let blended = match (nnue, hce) {
                (Some(nnue_eval), Some(hce_eval)) => blended_eval(nnue_eval, hce_eval, blend),
                (Some(nnue_eval), None) => nnue_eval,
                (None, Some(hce_eval)) => hce_eval,
                (None, None) => 0,
            };
            println!(
                "info string eval hce {hce} nnue {nnue} blend {blend} blended {blended}",
                hce = hce.map_or_else(|| "none".to_string(), |v| v.to_string()),
                nnue = nnue.map_or_else(|| "none".to_string(), |v| v.to_string())
            );
        });
    }

    fn handle_eval_features(&self) {
        let features = self.engine.board().hce_feature_breakdown();
        let names = crate::board::HCE_FEATURE_NAMES.join(",");
        let values = features
            .values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "info string evalfeatures names {names} values {values} tempo {tempo} phase {phase} full_white {full_white} tuned_full_white {tuned_full_white}",
            tempo = features.tempo,
            phase = features.phase_score,
            full_white = features.full_white,
            tuned_full_white = features.tuned_full_white
        );
    }

    fn handle_unknown(&self, line: &str) {
        if self.state.debug {
            eprintln!("Unknown command: {line}");
            eprintln!("Known commands: {}", KNOWN_COMMANDS.join(", "));
        }
    }

    /// Process a single UCI command. Returns false if the engine should quit.
    fn handle_command(&mut self, cmd: UciCommand) -> bool {
        match cmd {
            UciCommand::Uci => {
                self.engine
                    .with_search_state_ref(|state| self.options.print(state));
            }
            UciCommand::IsReady => {
                print_ready();
            }
            UciCommand::UciNewGame => {
                self.engine.new_game();
            }
            UciCommand::Position(line) => {
                self.handle_position(&line);
            }
            UciCommand::Perft(depth) => {
                self.handle_perft(depth);
            }
            UciCommand::Go(params) => {
                self.handle_go(&params);
            }
            UciCommand::Eval => {
                self.handle_eval();
            }
            UciCommand::EvalFeatures => {
                self.handle_eval_features();
            }
            UciCommand::Stop => {
                self.engine.signal_stop();
            }
            UciCommand::PonderHit => {
                self.engine.ponderhit();
            }
            UciCommand::SetOption { name, value } => {
                self.handle_setoption(&name, value.as_deref());
            }
            UciCommand::Debug(value) => {
                self.state.debug = matches!(value.as_deref(), Some("on"));
                self.engine.set_trace(self.state.debug);
            }
            UciCommand::Quit => {
                self.engine.stop_search();
                return false;
            }
            UciCommand::Unknown(line) => {
                self.handle_unknown(&line);
            }
        }
        true
    }
}

pub fn run_uci_session<R: BufRead>(first_line: Option<String>, reader: R) {
    let mut stdout = io::stdout();
    let mut session = UciSession::new(DEFAULT_TT_MB);

    if let Some(line) = first_line {
        if let Some(cmd) = parse_uci_command(&line) {
            if !session.handle_command(cmd) {
                return;
            }
        }
        let _ = stdout.flush();
    }

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if let Some(cmd) = parse_uci_command(&line) {
            let keep_running = session.handle_command(cmd);
            if !keep_running {
                break;
            }
        }

        let _ = stdout.flush();
    }
}

pub fn run_uci() {
    let stdin = io::stdin();
    run_uci_session(None, stdin.lock());
}

#[cfg(test)]
mod tests {
    use super::{blended_eval, scaled_eval};

    #[test]
    fn scaled_eval_applies_percent_scale() {
        assert_eq!(scaled_eval(200, 50), 100);
        assert_eq!(scaled_eval(-200, 125), -250);
    }

    #[test]
    fn blended_eval_applies_nnue_weight() {
        assert_eq!(blended_eval(100, 300, 100), 100);
        assert_eq!(blended_eval(100, 300, 0), 300);
        assert_eq!(blended_eval(100, 300, 25), 250);
    }
}
