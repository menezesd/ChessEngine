use std::env;

#[derive(Clone, Debug)]
pub struct SearchParams {
    pub null_min_depth: u32,
    pub null_reduction: u32,
    pub null_verification_depth: u32,
    pub iir_min_depth: u32,
    pub razor_margin: i32,
    pub singular_margin: i32,
    pub rfp_margin: i32,
    pub static_null_margin: i32,
    pub lmp_min_depth: u32,
    pub lmp_move_limit: usize,
    pub futility_margin: i32,
    pub lmr_min_depth: u32,
    pub lmr_min_move: usize,
    pub lmr_reduction: u32,
    pub delta_margin: i32,
}

impl SearchParams {
    /// Baseline (aggressive) search parameters
    fn baseline() -> Self {
        SearchParams {
            null_min_depth: 3,
            null_reduction: 2,
            null_verification_depth: 8,
            iir_min_depth: 4,
            razor_margin: 250,
            singular_margin: 50,
            rfp_margin: 70,
            static_null_margin: 100,
            lmp_min_depth: 2,
            lmp_move_limit: 8,
            futility_margin: 100,
            lmr_min_depth: 3,
            lmr_min_move: 3,
            lmr_reduction: 1,
            delta_margin: 50,
        }
    }

    /// Conservative search parameters (safer pruning thresholds)
    fn conservative() -> Self {
        let mut params = Self::baseline();
        params.null_min_depth = 4;
        params.iir_min_depth = 6;
        params.razor_margin = 300;
        params.rfp_margin = 160;
        params.static_null_margin = 120;
        params.lmp_min_depth = 3;
        params.lmp_move_limit = 12;
        params.futility_margin = 140;
        params.lmr_min_depth = 4;
        params.lmr_min_move = 4;
        params.delta_margin = 60;
        params
    }
}

impl Default for SearchParams {
    fn default() -> Self {
        let style = env::var("SEARCH_STYLE")
            .unwrap_or_else(|_| String::new())
            .trim()
            .to_ascii_lowercase();
        if style == "baseline" {
            Self::baseline()
        } else {
            Self::conservative()
        }
    }
}
