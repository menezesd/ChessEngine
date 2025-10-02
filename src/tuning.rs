use crate::bench::{BenchSuite, SearchConfig};

pub struct TuningSession {
    suite: BenchSuite,
    base_config: SearchConfig,
    depth: u32,
    hash_mb: usize,
}

impl TuningSession {
    pub fn new(depth: u32, hash_mb: usize) -> Self {
        TuningSession {
            suite: BenchSuite::new(),
            base_config: SearchConfig::default(),
            depth,
            hash_mb,
        }
    }
    
    pub fn tune_lmr(&mut self) {
        println!("=== Tuning LMR Parameters ===");
        let base_nodes = self.get_baseline_nodes();
        
        // Test different LMR base reductions
        let reductions = [1, 2, 3];
        for &r in &reductions {
            let mut config = self.base_config.clone();
            config.lmr_base_reduction = r;
            
            let nodes = self.test_config(&config, &format!("LMR_R{}", r));
            let reduction = (base_nodes as f64 - nodes as f64) / base_nodes as f64 * 100.0;
            println!("LMR base reduction {}: {} nodes ({:+.1}%)", r, nodes, -reduction);
        }
        
        // Test different move thresholds
        let thresholds = [2, 3, 4, 6];
        for &t in &thresholds {
            let mut config = self.base_config.clone();
            config.lmr_move_threshold = t;
            
            let nodes = self.test_config(&config, &format!("LMR_T{}", t));
            let reduction = (base_nodes as f64 - nodes as f64) / base_nodes as f64 * 100.0;
            println!("LMR move threshold {}: {} nodes ({:+.1}%)", t, nodes, -reduction);
        }
    }
    
    pub fn tune_lmp(&mut self) {
        println!("\n=== Tuning LMP Parameters ===");
        let base_nodes = self.get_baseline_nodes();
        
        let thresholds = [8, 10, 12, 16, 20];
        for &t in &thresholds {
            let mut config = self.base_config.clone();
            config.lmp_move_threshold = t;
            
            let nodes = self.test_config(&config, &format!("LMP_T{}", t));
            let reduction = (base_nodes as f64 - nodes as f64) / base_nodes as f64 * 100.0;
            println!("LMP move threshold {}: {} nodes ({:+.1}%)", t, nodes, -reduction);
        }
    }
    
    pub fn tune_futility(&mut self) {
        println!("\n=== Tuning Futility Parameters ===");
        let base_nodes = self.get_baseline_nodes();
        
        let margins = [100, 125, 150, 175, 200];
        for &m in &margins {
            let mut config = self.base_config.clone();
            config.futility_margin_base = m;
            
            let nodes = self.test_config(&config, &format!("FUT_M{}", m));
            let reduction = (base_nodes as f64 - nodes as f64) / base_nodes as f64 * 100.0;
            println!("Futility margin {}: {} nodes ({:+.1}%)", m, nodes, -reduction);
        }
    }
    
    pub fn tune_null_move(&mut self) {
        println!("\n=== Tuning Null Move Parameters ===");
        let base_nodes = self.get_baseline_nodes();
        
        let reductions = [1, 2, 3];
        for &r in &reductions {
            let mut config = self.base_config.clone();
            config.null_move_r_base = r;
            
            let nodes = self.test_config(&config, &format!("NULL_R{}", r));
            let reduction = (base_nodes as f64 - nodes as f64) / base_nodes as f64 * 100.0;
            println!("Null move base reduction {}: {} nodes ({:+.1}%)", r, nodes, -reduction);
        }
    }
    
    pub fn find_optimal_config(&mut self) -> SearchConfig {
        println!("\n=== Finding Optimal Configuration ===");
        
        let mut best_config = self.base_config.clone();
        let mut best_nodes = self.get_baseline_nodes();
        
        // Grid search on key parameters
        let lmr_reductions = [1, 2];
        let lmr_thresholds = [3, 4, 6];
        let lmp_thresholds = [10, 12, 16];
        let futility_margins = [125, 150, 175];
        
        let total_combinations = lmr_reductions.len() * lmr_thresholds.len() * 
                                lmp_thresholds.len() * futility_margins.len();
        let mut tested = 0;
        
        for &lmr_r in &lmr_reductions {
            for &lmr_t in &lmr_thresholds {
                for &lmp_t in &lmp_thresholds {
                    for &fut_m in &futility_margins {
                        tested += 1;
                        
                        let mut config = self.base_config.clone();
                        config.lmr_base_reduction = lmr_r;
                        config.lmr_move_threshold = lmr_t;
                        config.lmp_move_threshold = lmp_t;
                        config.futility_margin_base = fut_m;
                        
                        let nodes = self.test_config(&config, &format!("Test{}", tested));
                        
                        if nodes < best_nodes {
                            best_nodes = nodes;
                            best_config = config;
                            println!("New best: {} nodes (LMR R{}/T{}, LMP T{}, FUT M{})", 
                                nodes, lmr_r, lmr_t, lmp_t, fut_m);
                        }
                        
                        if tested % 5 == 0 {
                            println!("Progress: {}/{} configurations tested", tested, total_combinations);
                        }
                    }
                }
            }
        }
        
        println!("\nOptimal configuration found:");
        println!("LMR base reduction: {}", best_config.lmr_base_reduction);
        println!("LMR move threshold: {}", best_config.lmr_move_threshold);
        println!("LMP move threshold: {}", best_config.lmp_move_threshold);
        println!("Futility margin: {}", best_config.futility_margin_base);
        println!("Total nodes: {} ({:.1}% reduction)", 
            best_nodes, 
            (self.get_baseline_nodes() as f64 - best_nodes as f64) / self.get_baseline_nodes() as f64 * 100.0);
        
        best_config
    }
    
    fn get_baseline_nodes(&self) -> u64 {
        let results = self.suite.run_bench(&[self.depth], self.hash_mb);
        results.iter().map(|r| r.nodes).sum()
    }
    
    fn test_config(&self, config: &SearchConfig, name: &str) -> u64 {
        let results = self.suite.compare_configs(self.depth, self.hash_mb, vec![(name, config.clone())]);
        results[0].total_nodes
    }
}

pub fn run_tuning_command(args: &[String]) {
    let depth = if args.is_empty() { 5 } else { args[0].parse().unwrap_or(5) };
    let mut session = TuningSession::new(depth, 64);
    
    if args.len() > 1 {
        match args[1].as_str() {
            "lmr" => session.tune_lmr(),
            "lmp" => session.tune_lmp(),
            "futility" => session.tune_futility(),
            "nullmove" => session.tune_null_move(),
            "optimal" => { session.find_optimal_config(); },
            _ => {
                println!("Available tuning options: lmr, lmp, futility, nullmove, optimal");
                return;
            }
        }
    } else {
        // Run all tuning sessions
        session.tune_lmr();
        session.tune_lmp();
        session.tune_futility();
        session.tune_null_move();
        session.find_optimal_config();
    }
}