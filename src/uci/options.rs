use crate::board::SearchParams;

mod apply;
mod output;
mod parse;

use output::{print_check, print_spin, print_string};

pub use parse::parse_setoption;

pub(super) const HASH_MIN_MB: usize = 1;
pub(super) const HASH_MAX_MB: usize = 65_536;
pub(super) const THREADS_MIN: usize = 1;
pub(super) const THREADS_MAX: usize = 256;
pub(super) const MOVE_OVERHEAD_MIN_MS: u64 = 0;
pub(super) const MOVE_OVERHEAD_MAX_MS: u64 = 1000;
pub(super) const SOFT_TIME_PERCENT_MIN: u64 = 1;
pub(super) const SOFT_TIME_PERCENT_MAX: u64 = 100;
pub(super) const HARD_TIME_PERCENT_MIN: u64 = 1;
pub(super) const HARD_TIME_PERCENT_MAX: u64 = 100;
pub(super) const MAX_NODES_MIN: u64 = 0;
pub(super) const MULTIPV_MIN: u32 = 1;
pub(super) const MULTIPV_MAX: u32 = 64;
pub(super) const NNUE_SCALE_MIN: i32 = 25;
pub(super) const NNUE_SCALE_MAX: i32 = 400;
pub(super) const NNUE_BLEND_MIN: i32 = 0;
pub(super) const NNUE_BLEND_MAX: i32 = 100;
pub(super) const RFP_MARGIN_MIN: i32 = 50;
pub(super) const RFP_MARGIN_MAX: i32 = 300;
pub(super) const NULL_REDUCTION_MIN: u32 = 1;
pub(super) const NULL_REDUCTION_MAX: u32 = 5;
pub(super) const FUTILITY_MARGIN_MIN: i32 = 50;
pub(super) const FUTILITY_MARGIN_MAX: i32 = 250;
pub(super) const IIR_MIN_DEPTH_MIN: u32 = 3;
pub(super) const IIR_MIN_DEPTH_MAX: u32 = 8;
pub(super) const LMR_MIN_DEPTH_MIN: u32 = 2;
pub(super) const LMR_MIN_DEPTH_MAX: u32 = 6;

#[derive(Clone, Copy)]
pub enum UciOptionAction {
    ReinitHash(usize),
    SetThreads(usize),
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
            SOFT_TIME_PERCENT_MIN,
            SOFT_TIME_PERCENT_MAX,
        );
        print_spin(
            "Hard Time Percent",
            self.hard_time_percent,
            HARD_TIME_PERCENT_MIN,
            HARD_TIME_PERCENT_MAX,
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
        print_spin("NnueHceBlend", 100, NNUE_BLEND_MIN, NNUE_BLEND_MAX);
        print_check("NnuePureStaticEval", false);
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

    #[test]
    fn setoption_nnue_pure_static_eval_ignores_missing_or_invalid_value() {
        let mut options = UciOptions::new(16);
        let mut state = SearchState::new(16);

        options.apply_setoption("NnuePureStaticEval", Some("true"), &mut state);
        assert!(state.nnue_pure_static_eval);

        options.apply_setoption("NnuePureStaticEval", Some("maybe"), &mut state);
        assert!(state.nnue_pure_static_eval);

        options.apply_setoption("NnuePureStaticEval", None, &mut state);
        assert!(state.nnue_pure_static_eval);

        options.apply_setoption("NnuePureStaticEval", Some("false"), &mut state);
        assert!(!state.nnue_pure_static_eval);
    }
}
