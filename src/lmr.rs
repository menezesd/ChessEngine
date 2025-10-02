/// Late Move Reduction tables and logic 
pub struct LmrTable {
    // [is_pv][depth][move_number] -> reduction
    pub table: [[[u32; 64]; 64]; 2],
}

impl LmrTable {
    pub fn new() -> Self {
        let mut lmr = LmrTable {
            table: [[[0; 64]; 64]; 2],
        };
        
        lmr.init_table();
        lmr
    }
    
    fn init_table(&mut self) {
        // Initialize LMR table with logarithmic formula
        // Formula: 0.38 + log(depth) * log(moves) / 2.17
        for depth in 0..64 {
            for moves in 0..64 {
                let reduction = if depth == 0 || moves == 0 {
                    0
                } else {
                    let r = 0.38 + (depth as f64).ln() * (moves as f64).ln() / 2.17;
                    r as u32
                };
                
                // Zero window nodes (non-PV) get slightly more reduction
                self.table[0][depth][moves] = if reduction == 0 { 0 } else { reduction + 1 };
                // PV nodes get base reduction
                self.table[1][depth][moves] = reduction;
            }
        }
    }
    
    /// Get LMR reduction amount
    pub fn get_reduction(
        &self,
        is_pv: bool,
        depth: u32,
        move_count: usize,
        is_improving: bool,
        gives_check: bool,
        is_killer: bool,
        history_score: i32
    ) -> u32 {
        if depth < 3 || move_count == 0 {
            return 0;
        }
        
        let depth_idx = (depth as usize).min(63);
        let move_idx = move_count.min(63);
        let pv_idx = if is_pv { 1 } else { 0 };
        
        let mut reduction = self.table[pv_idx][depth_idx][move_idx];
        
        // Reduce less in PV nodes when we have a good move
        if is_pv && move_count <= 3 {
            reduction = reduction.saturating_sub(1);
        }
        
        // Reduce less when position is improving
        if is_improving && reduction > 1 {
            reduction = reduction.saturating_sub(1);
        }
        
        // Reduce less for checking moves
        if gives_check && reduction > 1 {
            reduction = reduction.saturating_sub(1);
        }
        
        // Reduce less for killer moves
        if is_killer && reduction > 1 {
            reduction = reduction.saturating_sub(1);
        }
        
        // Reduce more for moves with poor history
        if history_score < 0 && reduction < depth - 1 {
            reduction += 1;
        }
        
        // Reduce more for very late moves
        if move_count >= 12 && reduction < depth - 1 {
            reduction += 1;
        }
        
        // Ensure reduction doesn't exceed depth - 1
        reduction.min(depth - 1)
    }
}

/// Improving position detection based on evaluation trend
pub fn is_improving(current_eval: i32, two_ply_ago_eval: i32, ply: usize) -> bool {
    ply <= 1 || current_eval >= two_ply_ago_eval
}

/// Enhanced Late Move Pruning thresholds
pub fn lmp_threshold(depth: u32, is_improving: bool) -> usize {
    let base = match depth {
        1 => 3,
        2 => 6,
        3 => 12,
        4 => 18,
        _ => 25,
    };
    
    if is_improving {
        base + 3
    } else {
        base
    }
}

/// Check extension logic
pub fn should_extend_check(depth: u32, is_pv: bool, gives_check: bool) -> bool {
    gives_check && (is_pv || depth < 4)
}

/// Recapture extension logic  
pub fn should_extend_recapture(
    ply: usize,
    is_pv: bool,
    depth: u32,
    current_to: crate::Square,
    previous_capture_to: Option<crate::Square>
) -> bool {
    if ply == 0 || depth >= 7 {
        return false;
    }
    
    if let Some(prev_to) = previous_capture_to {
        current_to == prev_to && (is_pv || depth < 7)
    } else {
        false
    }
}

/// Singular extension margin calculation
pub fn singular_extension_margin(depth: u32) -> i32 {
    (depth as i32) * 4
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lmr_table_initialization() {
        let lmr = LmrTable::new();
        
        // Test that first move and first depth get no reduction
        assert_eq!(lmr.table[0][0][0], 0);
        assert_eq!(lmr.table[1][0][0], 0);
        assert_eq!(lmr.table[0][1][0], 0);
        assert_eq!(lmr.table[1][1][0], 0);
        
        // Test that deeper/later moves get more reduction
        assert!(lmr.table[0][10][10] > lmr.table[0][5][5]);
        assert!(lmr.table[0][10][10] > lmr.table[1][10][10]); // Non-PV gets more reduction
    }
    
    #[test]
    fn test_lmp_threshold() {
        assert!(lmp_threshold(3, true) > lmp_threshold(3, false));
        assert!(lmp_threshold(4, false) > lmp_threshold(2, false));
    }
    
    #[test]
    fn test_improving_detection() {
        assert!(is_improving(100, 90, 2));
        assert!(!is_improving(90, 100, 2));
        assert!(is_improving(50, 100, 1)); // Always true for ply <= 1
    }
}