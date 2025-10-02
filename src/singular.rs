use crate::{Board, Move, TranspositionTable};
use crate::search::SearchHeuristics;

/// Singular extension implementation 
pub struct SingularExtension;

impl SingularExtension {
    const SINGULAR_DEPTH_THRESHOLD: u32 = 7;
    
    /// Check if we should attempt singular extension
    pub fn should_try_singular(
        depth: u32,
        is_root: bool,
        tt_move: Option<&Move>,
        excluded_move: Option<Move>
    ) -> bool {
        !is_root 
            && depth > Self::SINGULAR_DEPTH_THRESHOLD
            && tt_move.is_some()
            && excluded_move.is_none()
    }
    
    /// Perform singular extension search
    pub fn try_singular_extension(
        board: &mut Board,
        tt: &mut TranspositionTable,
        tt_move: Move,
        tt_score: i32,
        depth: u32,
    _alpha: i32,
    _beta: i32,
        heur: &mut SearchHeuristics,
        ply: usize
    ) -> bool {
        // Verify we have a lower bound entry with reasonable score
        if tt_score >= crate::MATE_SCORE - 1000 {
            return false;
        }
        
        // Set up exclusion search parameters
        let margin = Self::singular_margin(depth);
        let search_alpha = tt_score - margin;
        let search_beta = tt_score - margin + 1;
        let search_depth = (depth - 1) / 2;
        
        // Store the excluded move
        let original_excluded = heur.last_moves.get(ply).cloned().flatten();
        if ply < heur.last_moves.len() {
            heur.last_moves[ply] = Some(tt_move);
        }
        
        // Search all moves except the TT move
        let moves = board.generate_moves();
        let mut best_score = -crate::MATE_SCORE;
        let mut found_alternative = false;
        
        for mv in moves {
            if mv == tt_move {
                continue; // Skip the singular move candidate
            }
            
            let info = board.make_move(&mv);
            
            // Check if move is legal
            if board.is_in_check(board.current_color()) {
                board.unmake_move(&mv, info);
                continue;
            }
            
            found_alternative = true;
            
            // Search with null window around the reduced bound
            let score = -board.negamax(
                tt, 
                search_depth, 
                -search_beta, 
                -search_alpha, 
                heur, 
                ply + 1
            );
            
            board.unmake_move(&mv, info);
            
            if heur.check_abort() {
                break;
            }
            
            best_score = best_score.max(score);
            
            // If any move gets close to the TT move's score, it's not singular
            if score > search_alpha {
                // Restore excluded move
                if ply < heur.last_moves.len() {
                    heur.last_moves[ply] = original_excluded;
                }
                return false;
            }
        }
        
        // Restore excluded move
        if ply < heur.last_moves.len() {
            heur.last_moves[ply] = original_excluded;
        }
        
        // Extend if we found alternatives but none came close to TT move score
        found_alternative && best_score <= search_alpha
    }
    
    /// Calculate singular extension margin based on depth
    fn singular_margin(depth: u32) -> i32 {
        (depth as i32) * 4
    }
}

/// Internal Iterative Reduction (IID) implementation
pub struct InternalIterativeReduction;

impl InternalIterativeReduction {
    /// Check if we should reduce depth due to missing TT move
    pub fn should_reduce(
        depth: u32,
        is_pv: bool,
        tt_move: Option<&Move>,
        in_check: bool
    ) -> bool {
        depth > 5 && !is_pv && tt_move.is_none() && !in_check
    }
    
    /// Get reduction amount for IID
    pub fn get_reduction() -> u32 {
        1
    }
}

/// Probcut implementation for very aggressive pruning
pub struct Probcut;

impl Probcut {
    const PROBCUT_DEPTH_THRESHOLD: u32 = 5;
    
    /// Check if we should try probcut
    pub fn should_try(
        depth: u32,
        is_pv: bool,
        in_check: bool,
        beta: i32
    ) -> bool {
        depth >= Self::PROBCUT_DEPTH_THRESHOLD
            && !is_pv
            && !in_check
            && beta.abs() < crate::MATE_SCORE - 500
    }
    
    /// Perform probcut search
    pub fn try_probcut(
        board: &mut Board,
        tt: &mut TranspositionTable,
        depth: u32,
        beta: i32,
        heur: &mut SearchHeuristics,
        ply: usize
    ) -> Option<i32> {
        let margin = Self::probcut_margin(depth);
        let probcut_beta = beta + margin;
        let probcut_depth = Self::probcut_depth(depth);
        
        // Generate captures and strong moves
        let moves = board.generate_tactical_moves();
        
        for mv in moves {
            // Only try moves that could potentially beat probcut_beta
            if board.see(&mv) < margin / 4 {
                continue;
            }
            
            let info = board.make_move(&mv);
            
            if board.is_in_check(board.current_color()) {
                board.unmake_move(&mv, info);
                continue;
            }
            
            let score = -board.negamax(
                tt,
                probcut_depth,
                -probcut_beta,
                -probcut_beta + 1,
                heur,
                ply + 1
            );
            
            board.unmake_move(&mv, info);
            
            if heur.check_abort() {
                break;
            }
            
            if score >= probcut_beta {
                return Some(score);
            }
        }
        
        None
    }
    
    fn probcut_margin(depth: u32) -> i32 {
        100 + 25 * (depth as i32)
    }
    
    fn probcut_depth(depth: u32) -> u32 {
        depth / 4 + 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_singular_extension_conditions() {
        let dummy_move = Move {
            from: crate::Square(0, 0),
            to: crate::Square(1, 1),
            is_castling: false,
            is_en_passant: false,
            promotion: None,
            captured_piece: None,
        };
        
        // Should try singular extension
        assert!(SingularExtension::should_try_singular(8, false, Some(&dummy_move), None));
        
        // Should not try at root
        assert!(!SingularExtension::should_try_singular(8, true, Some(&dummy_move), None));
        
        // Should not try with insufficient depth
        assert!(!SingularExtension::should_try_singular(6, false, Some(&dummy_move), None));
        
        // Should not try without TT move
        assert!(!SingularExtension::should_try_singular(8, false, None, None));
    }
    
    #[test]
    fn test_iir_conditions() {
        let dummy_move = Move {
            from: crate::Square(0, 0),
            to: crate::Square(1, 1),
            is_castling: false,
            is_en_passant: false,
            promotion: None,
            captured_piece: None,
        };
        
        // Should reduce when conditions are met
        assert!(InternalIterativeReduction::should_reduce(6, false, None, false));
        
        // Should not reduce in PV
        assert!(!InternalIterativeReduction::should_reduce(6, true, None, false));
        
        // Should not reduce when we have TT move
        assert!(!InternalIterativeReduction::should_reduce(6, false, Some(&dummy_move), false));
        
        // Should not reduce when in check
        assert!(!InternalIterativeReduction::should_reduce(6, false, None, true));
    }
    
    #[test]
    fn test_probcut_conditions() {
        // Should try probcut when conditions are met
        assert!(Probcut::should_try(6, false, false, 100));
        
        // Should not try in PV
        assert!(!Probcut::should_try(6, true, false, 100));
        
        // Should not try when in check
        assert!(!Probcut::should_try(6, false, true, 100));
        
        // Should not try with insufficient depth
        assert!(!Probcut::should_try(4, false, false, 100));
        
        // Should not try near mate scores
        assert!(!Probcut::should_try(6, false, false, crate::MATE_SCORE - 100));
    }
}