// Tuned search configuration based on benchmark results
// Generated from tuning session on standard test positions

use crate::bench::SearchConfig;

impl SearchConfig {
    // Optimized configuration based on tuning results:
    // - LMR base reduction 2: 27.3% node reduction 
    // - LMP threshold 8: 16.8% node reduction
    // - Conservative futility and null-move settings for tactical safety
    pub fn tuned_optimal() -> Self {
        SearchConfig {
            lmr_base_reduction: 2,           // Best from LMR tuning
            lmr_depth_threshold: 3,          // Standard
            lmr_move_threshold: 3,           // Standard
            lmp_depth_threshold: 3,          // Standard
            lmp_move_threshold: 8,           // Best from LMP tuning
            futility_depth_threshold: 2,     // Conservative for tactics
            futility_margin_base: 125,       // Moderate pruning
            null_move_r_base: 2,             // Moderate reduction
            null_move_depth_threshold: 3,    // Standard
            see_capture_threshold: 0,        // Standard
        }
    }
    
    // Balanced configuration with moderate improvements
    pub fn tuned_balanced() -> Self {
        SearchConfig {
            lmr_base_reduction: 2,           // Good node reduction
            lmr_depth_threshold: 3,
            lmr_move_threshold: 3,
            lmp_depth_threshold: 3,
            lmp_move_threshold: 10,          // Less aggressive than optimal
            futility_depth_threshold: 2,
            futility_margin_base: 150,       // Standard
            null_move_r_base: 2,
            null_move_depth_threshold: 3,
            see_capture_threshold: 0,
        }
    }
    
    // Speed-focused configuration for fast time controls
    pub fn tuned_fast() -> Self {
        SearchConfig {
            lmr_base_reduction: 3,           // More aggressive
            lmr_depth_threshold: 3,
            lmr_move_threshold: 2,           // Earlier LMR
            lmp_depth_threshold: 3,
            lmp_move_threshold: 6,           // More aggressive LMP
            futility_depth_threshold: 3,     // More futility
            futility_margin_base: 175,       // Higher margins
            null_move_r_base: 3,             // More reduction
            null_move_depth_threshold: 2,    // Earlier null-move
            see_capture_threshold: -50,      // Accept some bad captures for speed
        }
    }
}