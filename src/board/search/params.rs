#[derive(Clone, Debug)]
pub struct SearchParams {
    pub null_reduction: u32,
    pub null_min_depth: u32,
    pub null_verification_depth: u32,
    pub futility_margin: i32,
    pub razor_margin: i32,
    pub lmr_min_depth: u32,
    pub lmr_min_move: usize,
    pub lmr_reduction: u32,
    pub lmp_min_depth: u32,
    pub lmp_move_limit: usize,
    pub iir_min_depth: u32,
    pub singular_margin: i32,
    pub rfp_margin: i32,
    pub static_null_margin: i32,
    pub delta_margin: i32,
}

impl Default for SearchParams {
    fn default() -> Self {
        SearchParams {
            null_reduction: 2,
            null_min_depth: 3,
            null_verification_depth: 6,
            futility_margin: 150,
            razor_margin: 250,
            lmr_min_depth: 3,
            lmr_min_move: 3,
            lmr_reduction: 1,
            lmp_min_depth: 3,
            lmp_move_limit: 8,
            iir_min_depth: 6,
            singular_margin: 50,
            rfp_margin: 100,
            static_null_margin: 120,
            delta_margin: 200,
        }
    }
}
