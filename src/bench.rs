use crate::{Board, TranspositionTable};
use crate::search::SearchEngine;
use std::time::Instant;

pub struct BenchResult {
    pub position_name: String,
    pub depth: u32,
    pub nodes: u64,
    pub time_ms: u128,
    pub nps: u64,
    pub score: i32,
}

pub struct BenchSuite {
    positions: Vec<(&'static str, &'static str)>, // (name, fen)
}

impl BenchSuite {
    pub fn new() -> Self {
        BenchSuite {
            positions: vec![
                ("Initial", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
                ("Kiwipete", "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"),
                ("Position3", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
                ("Position4", "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"),
                ("Position5", "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"),
                ("Endgame1", "8/8/8/8/8/8/6k1/4K2R w K - 0 1"),
                ("Middlegame1", "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/3P1N2/PPP2PPP/RNBQK2R w KQkq - 0 1"),
                ("Tactical1", "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1"),
            ],
        }
    }

    pub fn run_bench(&self, depths: &[u32], hash_mb: usize) -> Vec<BenchResult> {
        let mut results = Vec::new();
        let mut tt = TranspositionTable::new(hash_mb);
        
        for &depth in depths {
            println!("Running bench at depth {}...", depth);
            for (name, fen) in &self.positions {
                tt.clear();
                let mut board = Board::from_fen(fen);
                let mut engine = SearchEngine::new();
                
                let start = Instant::now();
                let nodes_before = engine.timer.node_count;
                
                // Run search using think method with time limit
                let time_limit = Some(start + std::time::Duration::from_millis(5000)); // 5 second limit
                let _best_move = engine.think(&mut board, time_limit);
                
                let elapsed = start.elapsed();
                let nodes = engine.timer.node_count - nodes_before;
                let time_ms = elapsed.as_millis();
                let nps = if time_ms > 0 { (nodes as u128 * 1000 / time_ms) as u64 } else { nodes };
                
                let result = BenchResult {
                    position_name: name.to_string(),
                    depth,
                    nodes,
                    time_ms,
                    nps,
                    score: 0, // Engine doesn't expose final score directly
                };
                
                println!("  {:<12} depth {} nodes {:>8} time {:>4}ms nps {:>8}",
                    name, depth, nodes, time_ms, nps);
                
                results.push(result);
            }
        }
        
        results
    }
    
    pub fn compare_configs(&self, depth: u32, hash_mb: usize, configs: Vec<(&str, SearchConfig)>) -> Vec<ConfigResult> {
        let mut config_results = Vec::new();
        
        for (config_name, config) in configs {
            println!("\nTesting config: {}", config_name);
            let results = self.run_config_bench(depth, hash_mb, &config);
            let total_nodes: u64 = results.iter().map(|r| r.nodes).sum();
            let avg_nps: u64 = if !results.is_empty() {
                results.iter().map(|r| r.nps).sum::<u64>() / results.len() as u64
            } else {
                0
            };
            
            let config_result = ConfigResult {
                name: config_name.to_string(),
                total_nodes,
                avg_nps,
                results,
            };
            
            println!("Config {} - Total nodes: {}, Avg NPS: {}", config_name, total_nodes, avg_nps);
            config_results.push(config_result);
        }
        
        config_results
    }
    
    pub fn test_tuned_configs(&self, depth: u32, hash_mb: usize) -> Vec<ConfigResult> {
        let configs = vec![
            ("Default", SearchConfig::default()),
            ("Aggressive", SearchConfig::aggressive_pruning()),
            ("Conservative", SearchConfig::conservative_pruning()),
            ("Deep LMR", SearchConfig::deep_lmr()),
        ];
        
        self.compare_configs(depth, hash_mb, configs)
    }
    
    fn run_config_bench(&self, depth: u32, hash_mb: usize, config: &SearchConfig) -> Vec<BenchResult> {
        let mut results = Vec::new();
        let mut tt = TranspositionTable::new(hash_mb);
        
        println!("Running bench at depth {}...", depth);
        for (name, fen) in &self.positions {
            tt.clear();
            let mut board = Board::from_fen(fen);
            let mut engine = SearchEngine::new();
            
            let start = Instant::now();
            let nodes_before = engine.timer.node_count;
            
            // Run search with time limit (ignoring specific config for now)
            let time_limit = Some(start + std::time::Duration::from_millis(5000)); // 5 second limit
            let _best_move = engine.think(&mut board, time_limit);
            
            let elapsed = start.elapsed();
            let nodes = engine.timer.node_count - nodes_before;
            let time_ms = elapsed.as_millis();
            let nps = if time_ms > 0 { (nodes as u128 * 1000 / time_ms) as u64 } else { nodes };
            let best_score = 0; // Engine doesn't expose final score directly
            
            let result = BenchResult {
                position_name: name.to_string(),
                depth,
                nodes,
                time_ms,
                nps,
                score: best_score,
            };
            
            println!("  {:<12} depth {} nodes {:>8} time {:>4}ms nps {:>8}",
                name, depth, nodes, time_ms, nps);
            
            results.push(result);
        }
        
        results
    }
}

#[derive(Clone)]
pub struct SearchConfig {
    pub lmr_base_reduction: u32,
    pub lmr_depth_threshold: u32,
    pub lmr_move_threshold: usize,
    pub lmp_depth_threshold: u32,
    pub lmp_move_threshold: usize,
    pub futility_depth_threshold: u32,
    pub futility_margin_base: i32,
    pub null_move_r_base: u32,
    pub null_move_depth_threshold: u32,
    pub see_capture_threshold: i32,
}

impl SearchConfig {
    pub fn default() -> Self {
        SearchConfig {
            lmr_base_reduction: 1,
            lmr_depth_threshold: 3,
            lmr_move_threshold: 3,
            lmp_depth_threshold: 3,
            lmp_move_threshold: 12,
            futility_depth_threshold: 2,
            futility_margin_base: 150,
            null_move_r_base: 1,
            null_move_depth_threshold: 3,
            see_capture_threshold: 0,
        }
    }
    
    // Variations for tuning
    pub fn aggressive_pruning() -> Self {
        let mut config = Self::default();
        config.lmr_base_reduction = 2;
        config.lmp_move_threshold = 8;
        config.futility_margin_base = 200;
        config.null_move_r_base = 2;
        config
    }
    
    pub fn conservative_pruning() -> Self {
        let mut config = Self::default();
        config.lmr_base_reduction = 1;
        config.lmp_move_threshold = 16;
        config.futility_margin_base = 100;
        config.null_move_r_base = 1;
        config
    }
    
    pub fn deep_lmr() -> Self {
        let mut config = Self::default();
        config.lmr_base_reduction = 1;
        config.lmr_depth_threshold = 5;
        config.lmr_move_threshold = 6;
        config
    }
}

pub struct ConfigResult {
    pub name: String,
    pub total_nodes: u64,
    pub avg_nps: u64,
    pub results: Vec<BenchResult>,
}

// CLI interface for benchmarking
pub fn run_bench_command(args: &[String]) {
    let suite = BenchSuite::new();
    
    if args.is_empty() {
        // Default bench: depth 6, 64MB hash
        println!("Running default benchmark (depth 6, 64MB hash)...");
        let results = suite.run_bench(&[6], 64);
        print_bench_summary(&results);
    } else if args[0] == "compare" {
        // Compare different configurations
        println!("Comparing search configurations at depth 5...");
        let configs = vec![
            ("Default", SearchConfig::default()),
            ("Aggressive", SearchConfig::aggressive_pruning()),
            ("Conservative", SearchConfig::conservative_pruning()),
            ("Deep LMR", SearchConfig::deep_lmr()),
        ];
        
        let config_results = suite.compare_configs(5, 64, configs);
        print_config_comparison(&config_results);
    } else if args[0] == "tuned" {
        // Compare tuned configurations
        println!("Comparing tuned configurations at depth 6...");
        let config_results = suite.test_tuned_configs(6, 64);
        print_config_comparison(&config_results);
    } else if let Ok(depth) = args[0].parse::<u32>() {
        // Custom depth
        println!("Running benchmark at depth {}...", depth);
        let results = suite.run_bench(&[depth], 64);
        print_bench_summary(&results);
    } else {
        println!("Usage: bench [depth|compare|tuned]");
    }
}

fn print_bench_summary(results: &[BenchResult]) {
    let total_nodes: u64 = results.iter().map(|r| r.nodes).sum();
    let total_time: u128 = results.iter().map(|r| r.time_ms).sum();
    let avg_nps = if total_time > 0 { (total_nodes as u128 * 1000 / total_time) as u64 } else { 0 };
    
    println!("\n=== Benchmark Summary ===");
    println!("Total nodes: {}", total_nodes);
    println!("Total time: {}ms", total_time);
    println!("Average NPS: {}", avg_nps);
    
    if !results.is_empty() {
        let depth = results[0].depth;
        println!("Depth: {}", depth);
        println!("Positions: {}", results.len());
    }
}

fn print_config_comparison(config_results: &[ConfigResult]) {
    println!("\n=== Configuration Comparison ===");
    println!("{:<12} {:>10} {:>10}", "Config", "Nodes", "Avg NPS");
    println!("{}", "-".repeat(34));
    
    for result in config_results {
        println!("{:<12} {:>10} {:>10}", 
            result.name, 
            result.total_nodes, 
            result.avg_nps);
    }
    
    if let Some(baseline) = config_results.first() {
        println!("\nRelative to {}:", baseline.name);
        for result in config_results.iter().skip(1) {
            let node_ratio = result.total_nodes as f64 / baseline.total_nodes as f64;
            let nps_ratio = result.avg_nps as f64 / baseline.avg_nps as f64;
            println!("{:<12} nodes: {:.3}x nps: {:.3}x", 
                result.name, node_ratio, nps_ratio);
        }
    }
}