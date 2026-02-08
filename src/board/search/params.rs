use std::env;

#[derive(Clone, Debug)]
pub struct SearchParams {
    pub null_min_depth: u32,
    pub null_reduction: u32,
    pub null_verification_depth: u32,
    pub iir_min_depth: u32,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_params() {
        let params = SearchParams::baseline();
        // Baseline has more aggressive pruning
        assert!(params.null_min_depth <= 3);
        assert!(params.iir_min_depth <= 4);
        assert!(params.lmp_min_depth <= 2);
    }

    #[test]
    fn test_conservative_params() {
        let params = SearchParams::conservative();
        let baseline = SearchParams::baseline();

        // Conservative should have higher thresholds (less pruning)
        assert!(params.null_min_depth >= baseline.null_min_depth);
        assert!(params.rfp_margin >= baseline.rfp_margin);
        assert!(params.lmp_min_depth >= baseline.lmp_min_depth);
        assert!(params.lmp_move_limit >= baseline.lmp_move_limit);
    }

    #[test]
    fn test_params_clone() {
        let params = SearchParams::baseline();
        let cloned = params.clone();
        assert_eq!(cloned.null_min_depth, params.null_min_depth);
        assert_eq!(cloned.lmr_reduction, params.lmr_reduction);
    }

    #[test]
    fn test_null_move_params() {
        let params = SearchParams::baseline();
        // Null move should require some minimum depth
        assert!(params.null_min_depth >= 2);
        // Reduction should be reasonable
        assert!(params.null_reduction >= 1 && params.null_reduction <= 4);
    }

    #[test]
    fn test_lmr_params() {
        let params = SearchParams::baseline();
        // LMR should start after a few moves
        assert!(params.lmr_min_move >= 2);
        // LMR should require some minimum depth
        assert!(params.lmr_min_depth >= 2);
    }

    #[test]
    fn test_futility_margin() {
        let params = SearchParams::baseline();
        // Futility margin should be roughly pawn value
        assert!(params.futility_margin >= 50 && params.futility_margin <= 200);
    }
}
