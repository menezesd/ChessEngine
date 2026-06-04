use crate::board::{SearchParams, SearchState};

mod output;
mod parse;

use output::{print_check, print_spin, print_string};

pub use parse::parse_setoption;

const HASH_MIN_MB: usize = 1;
const HASH_MAX_MB: usize = 65_536;
const THREADS_MIN: usize = 1;
const THREADS_MAX: usize = 256;
const MOVE_OVERHEAD_MIN_MS: u64 = 0;
const MOVE_OVERHEAD_MAX_MS: u64 = 1000;
const TIME_PERCENT_MIN: u64 = 1;
const TIME_PERCENT_MAX: u64 = 100;
const MAX_NODES_MIN: u64 = 0;
const MULTIPV_MIN: u32 = 1;
const MULTIPV_MAX: u32 = 64;
const NNUE_SCALE_MIN: i32 = 25;
const NNUE_SCALE_MAX: i32 = 400;
const NNUE_BLEND_MIN: i32 = 0;
const NNUE_BLEND_MAX: i32 = 100;
const RFP_MARGIN_MIN: i32 = 50;
const RFP_MARGIN_MAX: i32 = 300;
const NULL_REDUCTION_MIN: u32 = 1;
const NULL_REDUCTION_MAX: u32 = 5;
const FUTILITY_MARGIN_MIN: i32 = 50;
const FUTILITY_MARGIN_MAX: i32 = 250;
const IIR_MIN_DEPTH_MIN: u32 = 3;
const IIR_MIN_DEPTH_MAX: u32 = 8;
const LMR_MIN_DEPTH_MIN: u32 = 2;
const LMR_MIN_DEPTH_MAX: u32 = 6;

#[derive(Clone, Copy)]
pub enum UciOptionAction {
    ReinitHash(usize),
    SetThreads(usize),
}

enum ApplyOptionResult {
    Handled(Option<UciOptionAction>),
    Unhandled,
}

pub struct UciOptions {
    pub hash_mb: usize,
    pub threads: usize,
    pub default_max_nodes: u64,
    pub move_overhead_ms: u64,
    pub soft_time_percent: u64,
    pub hard_time_percent: u64,
    pub multi_pv: u32,
    pub ponder: bool,
    pub use_nnue: bool,
    pub eval_file: String,
    pub static_eval_file: String,
}

impl UciOptions {
    #[must_use]
    pub fn new(hash_mb: usize) -> Self {
        UciOptions {
            hash_mb,
            threads: 1,
            default_max_nodes: 0,
            move_overhead_ms: 50,
            soft_time_percent: 70,
            hard_time_percent: 90,
            multi_pv: 1,
            ponder: false,
            use_nnue: true,
            eval_file: String::new(),
            static_eval_file: String::new(),
        }
    }

    pub fn print(&self, params: &SearchParams) {
        println!("id name chess_engine");
        println!("id author Dean Menezes");

        self.print_engine_options();
        self.print_nnue_options();
        Self::print_search_options(params);

        println!("uciok");
    }

    fn print_engine_options(&self) {
        print_spin("Hash", self.hash_mb, HASH_MIN_MB, HASH_MAX_MB);
        print_spin("Threads", self.threads, THREADS_MIN, THREADS_MAX);
        print_spin(
            "Move Overhead",
            self.move_overhead_ms,
            MOVE_OVERHEAD_MIN_MS,
            MOVE_OVERHEAD_MAX_MS,
        );
        print_spin(
            "Soft Time Percent",
            self.soft_time_percent,
            TIME_PERCENT_MIN,
            TIME_PERCENT_MAX,
        );
        print_spin(
            "Hard Time Percent",
            self.hard_time_percent,
            TIME_PERCENT_MIN,
            TIME_PERCENT_MAX,
        );
        print_spin("Max Nodes", self.default_max_nodes, MAX_NODES_MIN, u64::MAX);
        print_spin("MultiPV", self.multi_pv, MULTIPV_MIN, MULTIPV_MAX);
        print_check("Ponder", self.ponder);
    }

    fn print_nnue_options(&self) {
        print_check("UseNNUE", self.use_nnue);
        print_string("EvalFile", &self.eval_file);
        print_string("StaticEvalFile", &self.static_eval_file);
        print_spin("NnueEvalScale", 100, NNUE_SCALE_MIN, NNUE_SCALE_MAX);
        print_spin("NnueStaticEvalScale", 100, NNUE_SCALE_MIN, NNUE_SCALE_MAX);
        print_spin("NnueStaticBlend", 100, NNUE_BLEND_MIN, NNUE_BLEND_MAX);
    }

    fn print_search_options(params: &SearchParams) {
        print_spin(
            "RFPMargin",
            params.rfp_margin,
            RFP_MARGIN_MIN,
            RFP_MARGIN_MAX,
        );
        print_spin(
            "NullMoveReduction",
            params.null_reduction,
            NULL_REDUCTION_MIN,
            NULL_REDUCTION_MAX,
        );
        print_spin(
            "FutilityMargin",
            params.futility_margin,
            FUTILITY_MARGIN_MIN,
            FUTILITY_MARGIN_MAX,
        );
        print_spin(
            "IIRMinDepth",
            params.iir_min_depth,
            IIR_MIN_DEPTH_MIN,
            IIR_MIN_DEPTH_MAX,
        );
        print_spin(
            "LMRMinDepth",
            params.lmr_min_depth,
            LMR_MIN_DEPTH_MIN,
            LMR_MIN_DEPTH_MAX,
        );
    }

    fn parsed_clamped<T>(value: Option<&str>, min: T, max: T) -> Option<T>
    where
        T: std::str::FromStr + Ord,
    {
        value
            .and_then(|v| v.parse::<T>().ok())
            .map(|v| v.clamp(min, max))
    }

    fn parsed<T>(value: Option<&str>) -> Option<T>
    where
        T: std::str::FromStr,
    {
        value.and_then(|v| v.parse::<T>().ok())
    }

    fn assign_parsed<T>(target: &mut T, value: Option<&str>)
    where
        T: std::str::FromStr,
    {
        if let Some(v) = Self::parsed(value) {
            *target = v;
        }
    }

    fn assign_parsed_clamped<T>(target: &mut T, value: Option<&str>, min: T, max: T)
    where
        T: std::str::FromStr + Ord,
    {
        if let Some(v) = Self::parsed_clamped(value, min, max) {
            *target = v;
        }
    }

    fn parsed_bool(value: Option<&str>) -> Option<bool> {
        match value?.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }

    fn reload_nnue_files(&self, state: &mut SearchState) {
        if !self.eval_file.is_empty() {
            let _ = state.load_nnue(&self.eval_file);
        }
        if !self.static_eval_file.is_empty() {
            let _ = state.load_static_nnue(&self.static_eval_file);
        }
    }

    fn apply_use_nnue(&mut self, value: Option<&str>, state: &mut SearchState) {
        let Some(use_nnue) = Self::parsed_bool(value) else {
            return;
        };

        self.use_nnue = use_nnue;
        if self.use_nnue {
            self.reload_nnue_files(state);
        } else {
            state.tables.nnue = None;
            state.tables.static_nnue = None;
        }
    }

    fn set_eval_file(&mut self, value: Option<&str>, state: &mut SearchState) {
        if Self::assign_option_string(&mut self.eval_file, value) && self.use_nnue {
            let _ = state.load_nnue(&self.eval_file);
        }
    }

    fn set_static_eval_file(&mut self, value: Option<&str>, state: &mut SearchState) {
        if Self::assign_option_string(&mut self.static_eval_file, value) && self.use_nnue {
            let _ = state.load_static_nnue(&self.static_eval_file);
        }
    }

    fn assign_option_string(target: &mut String, value: Option<&str>) -> bool {
        let Some(value) = value else {
            return false;
        };

        *target = value.to_string();
        !target.is_empty()
    }

    fn apply_engine_option(&mut self, normalized: &str, value: Option<&str>) -> ApplyOptionResult {
        match normalized {
            "hash" => {
                let Some(mb) = Self::parsed_clamped(value, HASH_MIN_MB, HASH_MAX_MB) else {
                    return ApplyOptionResult::Handled(None);
                };
                if mb == self.hash_mb {
                    return ApplyOptionResult::Handled(None);
                }
                self.hash_mb = mb;
                ApplyOptionResult::Handled(Some(UciOptionAction::ReinitHash(mb)))
            }
            "threads" => {
                let Some(threads) = Self::parsed_clamped(value, THREADS_MIN, THREADS_MAX) else {
                    return ApplyOptionResult::Handled(None);
                };
                if threads == self.threads {
                    return ApplyOptionResult::Handled(None);
                }
                self.threads = threads;
                ApplyOptionResult::Handled(Some(UciOptionAction::SetThreads(threads)))
            }
            "move overhead" => {
                Self::assign_parsed_clamped(
                    &mut self.move_overhead_ms,
                    value,
                    MOVE_OVERHEAD_MIN_MS,
                    MOVE_OVERHEAD_MAX_MS,
                );
                ApplyOptionResult::Handled(None)
            }
            "soft time percent" | "softtime" => {
                Self::assign_parsed_clamped(
                    &mut self.soft_time_percent,
                    value,
                    TIME_PERCENT_MIN,
                    TIME_PERCENT_MAX,
                );
                ApplyOptionResult::Handled(None)
            }
            "hard time percent" | "hardtime" => {
                Self::assign_parsed_clamped(
                    &mut self.hard_time_percent,
                    value,
                    TIME_PERCENT_MIN,
                    TIME_PERCENT_MAX,
                );
                ApplyOptionResult::Handled(None)
            }
            "max nodes" | "nodes" => {
                Self::assign_parsed(&mut self.default_max_nodes, value);
                ApplyOptionResult::Handled(None)
            }
            "multipv" => {
                Self::assign_parsed_clamped(&mut self.multi_pv, value, MULTIPV_MIN, MULTIPV_MAX);
                ApplyOptionResult::Handled(None)
            }
            "ponder" => {
                if let Some(ponder) = Self::parsed_bool(value) {
                    self.ponder = ponder;
                }
                ApplyOptionResult::Handled(None)
            }
            _ => ApplyOptionResult::Unhandled,
        }
    }

    fn apply_nnue_option(
        &mut self,
        normalized: &str,
        value: Option<&str>,
        state: &mut SearchState,
    ) -> bool {
        match normalized {
            "usennue" => self.apply_use_nnue(value, state),
            "evalfile" => self.set_eval_file(value, state),
            "staticevalfile" => self.set_static_eval_file(value, state),
            "nnueevalscale" => Self::assign_parsed_clamped(
                &mut state.nnue_eval_scale,
                value,
                NNUE_SCALE_MIN,
                NNUE_SCALE_MAX,
            ),
            "nnuestaticevalscale" => Self::assign_parsed_clamped(
                &mut state.nnue_static_eval_scale,
                value,
                NNUE_SCALE_MIN,
                NNUE_SCALE_MAX,
            ),
            "nnuestaticblend" => Self::assign_parsed_clamped(
                &mut state.nnue_static_blend,
                value,
                NNUE_BLEND_MIN,
                NNUE_BLEND_MAX,
            ),
            _ => return false,
        }
        true
    }

    fn apply_search_option(normalized: &str, value: Option<&str>, state: &mut SearchState) -> bool {
        match normalized {
            "rfpmargin" => Self::assign_parsed_clamped(
                &mut state.params_mut().rfp_margin,
                value,
                RFP_MARGIN_MIN,
                RFP_MARGIN_MAX,
            ),
            "nullmovereduction" => Self::assign_parsed_clamped(
                &mut state.params_mut().null_reduction,
                value,
                NULL_REDUCTION_MIN,
                NULL_REDUCTION_MAX,
            ),
            "futilitymargin" => Self::assign_parsed_clamped(
                &mut state.params_mut().futility_margin,
                value,
                FUTILITY_MARGIN_MIN,
                FUTILITY_MARGIN_MAX,
            ),
            "iirmindepth" => Self::assign_parsed_clamped(
                &mut state.params_mut().iir_min_depth,
                value,
                IIR_MIN_DEPTH_MIN,
                IIR_MIN_DEPTH_MAX,
            ),
            "lmrmindepth" => Self::assign_parsed_clamped(
                &mut state.params_mut().lmr_min_depth,
                value,
                LMR_MIN_DEPTH_MIN,
                LMR_MIN_DEPTH_MAX,
            ),
            _ => return false,
        }
        true
    }

    pub fn apply_setoption(
        &mut self,
        name: &str,
        value: Option<&str>,
        state: &mut SearchState,
    ) -> Option<UciOptionAction> {
        let normalized = name.trim().to_ascii_lowercase();
        match self.apply_engine_option(&normalized, value) {
            ApplyOptionResult::Handled(action) => return action,
            ApplyOptionResult::Unhandled => {}
        }
        if self.apply_nnue_option(&normalized, value, state) {
            return None;
        }
        Self::apply_search_option(&normalized, value, state);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        UciOptionAction, UciOptions, HASH_MAX_MB, HASH_MIN_MB, MOVE_OVERHEAD_MAX_MS,
        MOVE_OVERHEAD_MIN_MS,
    };
    use crate::board::SearchState;

    #[test]
    fn setoption_hash_is_clamped_to_advertised_bounds() {
        let mut options = UciOptions::new(16);
        let mut state = SearchState::new(16);

        let action = options.apply_setoption("Hash", Some("0"), &mut state);
        assert_eq!(options.hash_mb, HASH_MIN_MB);
        assert!(matches!(
            action,
            Some(UciOptionAction::ReinitHash(HASH_MIN_MB))
        ));

        let action = options.apply_setoption("Hash", Some("999999"), &mut state);
        assert_eq!(options.hash_mb, HASH_MAX_MB);
        assert!(matches!(
            action,
            Some(UciOptionAction::ReinitHash(HASH_MAX_MB))
        ));
    }

    #[test]
    fn setoption_move_overhead_is_clamped_to_advertised_bounds() {
        let mut options = UciOptions::new(16);
        let mut state = SearchState::new(16);

        options.apply_setoption("Move Overhead", Some("999999"), &mut state);
        assert_eq!(options.move_overhead_ms, MOVE_OVERHEAD_MAX_MS);

        options.apply_setoption("Move Overhead", Some("0"), &mut state);
        assert_eq!(options.move_overhead_ms, MOVE_OVERHEAD_MIN_MS);
    }

    #[test]
    fn setoption_hash_ignores_missing_or_invalid_value() {
        let mut options = UciOptions::new(16);
        let mut state = SearchState::new(16);

        assert!(options
            .apply_setoption("Hash", Some("not-a-number"), &mut state)
            .is_none());
        assert_eq!(options.hash_mb, 16);

        assert!(options.apply_setoption("Hash", None, &mut state).is_none());
        assert_eq!(options.hash_mb, 16);
    }

    #[test]
    fn setoption_threads_ignores_missing_or_invalid_value() {
        let mut options = UciOptions::new(16);
        let mut state = SearchState::new(16);

        assert!(options
            .apply_setoption("Threads", Some("not-a-number"), &mut state)
            .is_none());
        assert_eq!(options.threads, 1);

        assert!(options
            .apply_setoption("Threads", None, &mut state)
            .is_none());
        assert_eq!(options.threads, 1);
    }

    #[test]
    fn setoption_ponder_ignores_missing_or_invalid_value() {
        let mut options = UciOptions::new(16);
        let mut state = SearchState::new(16);

        options.apply_setoption("Ponder", Some("true"), &mut state);
        assert!(options.ponder);

        options.apply_setoption("Ponder", Some("maybe"), &mut state);
        assert!(options.ponder);

        options.apply_setoption("Ponder", None, &mut state);
        assert!(options.ponder);

        options.apply_setoption("Ponder", Some("false"), &mut state);
        assert!(!options.ponder);
    }

    #[test]
    fn setoption_use_nnue_ignores_missing_or_invalid_value() {
        let mut options = UciOptions::new(16);
        let mut state = SearchState::new(16);

        options.apply_setoption("UseNNUE", Some("false"), &mut state);
        assert!(!options.use_nnue);

        options.apply_setoption("UseNNUE", Some("maybe"), &mut state);
        assert!(!options.use_nnue);

        options.apply_setoption("UseNNUE", None, &mut state);
        assert!(!options.use_nnue);

        options.apply_setoption("UseNNUE", Some("true"), &mut state);
        assert!(options.use_nnue);
    }
}
