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

impl Default for SearchParams {
    fn default() -> Self {
        let style = env::var("SEARCH_STYLE")
            .unwrap_or_else(|_| "baseline".to_string())
            .trim()
            .to_ascii_lowercase();
        if style == "conservative" {
            SearchParams {
                null_min_depth: 4,
                null_reduction: 2,
                null_verification_depth: 8,
                iir_min_depth: 6,
                razor_margin: 300,
                singular_margin: 50,
                rfp_margin: 160,
                static_null_margin: 120,
                lmp_min_depth: 3,
                lmp_move_limit: 12,
                futility_margin: 140,
                lmr_min_depth: 4,
                lmr_min_move: 4,
                lmr_reduction: 1,
                delta_margin: 60,
            }
        } else {
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
    }
}
