// Additional search optimizations for speed
use crate::{Board, Move};

// Enhanced pruning parameters
pub struct SearchOptimizations;

impl SearchOptimizations {
    // Late Move Reduction with more aggressive parameters
    #[inline(always)]
    pub fn calculate_lmr_reduction(
        depth: u32,
        move_index: usize,
        is_capture: bool,
        gives_check: bool,
        history_score: i32,
        is_pv: bool
    ) -> u32 {
        if depth < 3 || move_index == 0 || is_capture || gives_check {
            return 0;
        }
        
        let mut reduction = if depth >= 6 && move_index >= 6 { 2 } else { 1 };
        
        // Reduce more for bad history
        if history_score < 0 {
            reduction += 1;
        }
        
        // Reduce less in PV nodes
        if is_pv && reduction > 1 {
            reduction -= 1;
        }
        
        // Reduce more for very late moves
        if move_index >= 12 {
            reduction += 1;
        }
        
        reduction.min(depth - 1)
    }
    
    // Enhanced futility pruning
    #[inline(always)]
    pub fn futility_margin(depth: u32, improving: bool) -> i32 {
        let base = 150 * depth as i32;
        if improving { base + 50 } else { base }
    }
    
    // Reverse futility pruning (static null move pruning)
    #[inline(always)]
    pub fn reverse_futility_margin(depth: u32) -> i32 {
        120 * depth as i32
    }
    
    // SEE pruning threshold for captures
    #[inline(always)]
    pub fn see_capture_threshold(depth: u32) -> i32 {
        if depth <= 2 { -50 } else { 0 }
    }
    
    // SEE pruning threshold for quiet moves
    #[inline(always)]
    pub fn see_quiet_threshold(depth: u32) -> i32 {
        -30 * depth as i32
    }
    
    // Check if position is likely in zugzwang (for null move pruning)
    #[inline(always)]
    pub fn is_zugzwang_likely(board: &Board) -> bool {
        let current_color = board.current_color();
        let color_idx = if current_color == crate::board::Color::White { 0 } else { 1 };
        
        // Check material: likely zugzwang with only pawns + king or very few pieces
        // Count pieces by scanning the board
        let mut pawns = 0;
        let mut knights = 0;
        let mut bishops = 0;
        let mut rooks = 0;
        let mut queens = 0;
        
        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = board.squares[rank][file] {
                    if (color == crate::board::Color::White && color_idx == 0) ||
                       (color == crate::board::Color::Black && color_idx == 1) {
                        match piece {
                            crate::Piece::Pawn => pawns += 1,
                            crate::Piece::Knight => knights += 1,
                            crate::Piece::Bishop => bishops += 1,
                            crate::Piece::Rook => rooks += 1,
                            crate::Piece::Queen => queens += 1,
                            _ => {}
                        }
                    }
                }
            }
        }
        
        let total_pieces = knights + bishops + rooks + queens;
        
        // Zugzwang likely if only pawns + king, or very few pieces
        total_pieces == 0 || (total_pieces <= 2 && pawns == 0)
    }
    
    // Enhanced null move reduction
    #[inline(always)]
    pub fn null_move_reduction(depth: u32, piece_count: u32) -> u32 {
        let mut r = 3; // Base reduction
        
        if depth >= 8 {
            r += 1;
        }
        if depth >= 12 {
            r += 1;
        }
        if piece_count >= 8 {
            r += 1;
        }
        
        r
    }
    
    // Aspiration window sizes
    #[inline(always)]
    pub fn aspiration_window_size(depth: u32) -> i32 {
        if depth >= 5 {
            25 + (depth as i32 - 5) * 5
        } else {
            crate::MATE_SCORE // Full window for shallow depths
        }
    }
    
    // Check if we should try internal iterative deepening
    #[inline(always)]
    pub fn should_use_iid(depth: u32, tt_move: Option<&Move>, is_pv: bool) -> bool {
        depth >= 6 && tt_move.is_none() && is_pv
    }
    
    // Singular extension detection
    #[inline(always)]
    pub fn singular_extension_margin(depth: u32) -> i32 {
        (depth as i32) * 4
    }
    
    // Delta pruning in quiescence
    #[inline(always)]
    pub fn delta_pruning_margin() -> i32 {
        200 // Queen value margin
    }
    
    // Late move pruning thresholds
    #[inline(always)]
    pub fn lmp_threshold(depth: u32, improving: bool) -> usize {
        let base = match depth {
            1 => 3,
            2 => 6,
            3 => 12,
            _ => 20,
        };
        
        if improving { base + 2 } else { base }
    }
    
    // Probcut parameters
    #[inline(always)]
    pub fn probcut_margin(depth: u32) -> i32 {
        100 + 25 * depth as i32
    }
    
    #[inline(always)]
    pub fn probcut_depth(depth: u32) -> u32 {
        depth / 4 + 3
    }
}

// Fast position evaluation for pruning decisions
impl Board {
    // Quick tactical check - are there any tactical threats?
    pub fn has_tactical_threats(&self) -> bool {
        let current_color = self.current_color();
        
        // Check if any pieces are hanging
        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.squares[rank][file] {
                    if color == current_color && piece != crate::Piece::King {
                        let sq = crate::Square(rank, file);
                        // Simple check - just see if square is attacked
                        let defended = crate::bitboards::is_square_attacked_bb(&self.squares, sq, color);
                        if !defended {
                            // Piece is hanging, check if enemy can capture
                            let enemy_color = if color == crate::board::Color::White { 
                                crate::board::Color::Black 
                            } else { 
                                crate::board::Color::White 
                            };
                            
                            if crate::bitboards::is_square_attacked_bb(&self.squares, sq, enemy_color) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        
        false
    }
    
    // Rough position evaluation without expensive calculations
    pub fn evaluate_rough(&self) -> i32 {
        let mut score = 0;
        
        // Just material and very basic positional
        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.squares[rank][file] {
                    let piece_value = crate::piece_value(piece);
                    let positional_bonus = match piece {
                        crate::Piece::Pawn => {
                            // Encourage pawn advancement
                            if color == crate::board::Color::White {
                                rank as i32 * 5
                            } else {
                                (7 - rank) as i32 * 5
                            }
                        },
                        crate::Piece::Knight | crate::Piece::Bishop => {
                            // Center bonus
                            let center_dist = (rank as i32 - 3).abs() + (file as i32 - 3).abs();
                            10 - center_dist
                        },
                        _ => 0,
                    };
                    
                    if color == crate::board::Color::White {
                        score += piece_value + positional_bonus;
                    } else {
                        score -= piece_value + positional_bonus;
                    }
                }
            }
        }
        
        if !self.white_to_move {
            score = -score;
        }
        
        score
    }
}