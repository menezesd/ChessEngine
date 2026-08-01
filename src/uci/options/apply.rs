use crate::board::SearchState;

use super::{
    UciOptionAction, UciOptions, FUTILITY_MARGIN_MAX, FUTILITY_MARGIN_MIN, HARD_TIME_PERCENT_MAX,
    HARD_TIME_PERCENT_MIN, HASH_MAX_MB, HASH_MIN_MB, IIR_MIN_DEPTH_MAX, IIR_MIN_DEPTH_MIN,
    LMR_MIN_DEPTH_MAX, LMR_MIN_DEPTH_MIN, MAX_NODES_MIN, MOVE_OVERHEAD_MAX_MS,
    MOVE_OVERHEAD_MIN_MS, MULTIPV_MAX, MULTIPV_MIN, NNUE_BLEND_MAX, NNUE_BLEND_MIN, NNUE_SCALE_MAX,
    NNUE_SCALE_MIN, NULL_REDUCTION_MAX, NULL_REDUCTION_MIN, RFP_MARGIN_MAX, RFP_MARGIN_MIN,
    SOFT_TIME_PERCENT_MAX, SOFT_TIME_PERCENT_MIN, THREADS_MAX, THREADS_MIN,
};

enum ApplyOptionResult {
    Handled(Option<UciOptionAction>),
    Unhandled,
}

impl UciOptions {
    fn parsed_clamped<T>(value: Option<&str>, min: T, max: T) -> Option<T>
    where
        T: std::str::FromStr + Ord,
    {
        value
            .and_then(|v| v.parse::<T>().ok())
            .map(|v| v.clamp(min, max))
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
                    SOFT_TIME_PERCENT_MIN,
                    SOFT_TIME_PERCENT_MAX,
                );
                ApplyOptionResult::Handled(None)
            }
            "hard time percent" | "hardtime" => {
                Self::assign_parsed_clamped(
                    &mut self.hard_time_percent,
                    value,
                    HARD_TIME_PERCENT_MIN,
                    HARD_TIME_PERCENT_MAX,
                );
                ApplyOptionResult::Handled(None)
            }
            "max nodes" | "nodes" => {
                Self::assign_parsed_clamped(
                    &mut self.default_max_nodes,
                    value,
                    MAX_NODES_MIN,
                    u64::MAX,
                );
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
            "nnuehceblend" => Self::assign_parsed_clamped(
                &mut state.nnue_hce_blend,
                value,
                NNUE_BLEND_MIN,
                NNUE_BLEND_MAX,
            ),
            "nnuepurestaticeval" => {
                if let Some(value) = Self::parsed_bool(value) {
                    state.static_eval_options.nnue_pure = value;
                }
            }
            "usefullhce" => {
                if let Some(value) = Self::parsed_bool(value) {
                    state.hce_options.use_full = value;
                }
            }
            "usetunedhce" => {
                if let Some(value) = Self::parsed_bool(value) {
                    state.hce_options.use_tuned = value;
                }
            }
            "usefullhcestaticeval" => {
                if let Some(value) = Self::parsed_bool(value) {
                    state.static_eval_options.use_full_hce = value;
                }
            }
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
