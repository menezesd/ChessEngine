use crate::board::{SearchParams, SearchState, DEFAULT_TT_MB};

/// Print a UCI spin option.
fn print_spin(
    name: &str,
    default: impl std::fmt::Display,
    min: impl std::fmt::Display,
    max: impl std::fmt::Display,
) {
    println!("option name {name} type spin default {default} min {min} max {max}");
}

/// Print a UCI check option.
fn print_check(name: &str, default: bool) {
    println!(
        "option name {name} type check default {}",
        if default { "true" } else { "false" }
    );
}

pub enum UciOptionAction {
    ReinitHash(usize),
    SetThreads(usize),
}

pub struct UciOptions {
    pub hash_mb: usize,
    pub threads: usize,
    pub default_max_nodes: u64,
    pub move_overhead_ms: u64,
    pub soft_time_percent: u64,
    pub hard_time_percent: u64,
    pub multi_pv: u32,
    pub ponder: bool,
}

impl UciOptions {
    #[must_use]
    pub fn new(hash_mb: usize) -> Self {
        UciOptions {
            hash_mb,
            threads: 1,
            default_max_nodes: 0,
            move_overhead_ms: 50,
            soft_time_percent: 70,
            hard_time_percent: 90,
            multi_pv: 1,
            ponder: false,
        }
    }

    pub fn print(&self, params: &SearchParams) {
        println!("id name chess_engine");
        println!("id author Dean Menezes");

        // Engine options
        print_spin("Hash", self.hash_mb, 1, 65536);
        print_spin("Threads", self.threads, 1, 256);
        print_spin("Move Overhead", self.move_overhead_ms, 0, 1000);
        print_spin("Soft Time Percent", self.soft_time_percent, 1, 100);
        print_spin("Hard Time Percent", self.hard_time_percent, 1, 100);
        print_spin("Max Nodes", self.default_max_nodes, 0_u64, u64::MAX);
        print_spin("MultiPV", self.multi_pv, 1, 64);
        print_check("Ponder", self.ponder);

        // Tunable search parameters for SPSA
        print_spin("RFPMargin", params.rfp_margin, 50, 300);
        print_spin("NullMoveReduction", params.null_reduction, 1, 5);
        print_spin("FutilityMargin", params.futility_margin, 50, 250);
        print_spin("IIRMinDepth", params.iir_min_depth, 3, 8);
        print_spin("LMRMinDepth", params.lmr_min_depth, 2, 6);

        println!("uciok");
    }

    pub fn apply_setoption(
        &mut self,
        name: &str,
        value: Option<&str>,
        state: &mut SearchState,
    ) -> Option<UciOptionAction> {
        let normalized = name.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "hash" => {
                let mb = value
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(DEFAULT_TT_MB)
                    .max(1);
                if mb != self.hash_mb {
                    self.hash_mb = mb;
                    return Some(UciOptionAction::ReinitHash(mb));
                }
            }
            "threads" => {
                let threads = value
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 256);
                if threads != self.threads {
                    self.threads = threads;
                    return Some(UciOptionAction::SetThreads(threads));
                }
            }
            "move overhead" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.move_overhead_ms = v;
                }
            }
            "soft time percent" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.soft_time_percent = v.clamp(1, 100);
                }
            }
            "hard time percent" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.hard_time_percent = v.clamp(1, 100);
                }
            }
            "max nodes" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.default_max_nodes = v;
                }
            }
            "softtime" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.soft_time_percent = v.clamp(1, 100);
                }
            }
            "hardtime" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.hard_time_percent = v.clamp(1, 100);
                }
            }
            "nodes" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.default_max_nodes = v;
                }
            }
            "multipv" => {
                if let Some(v) = value.and_then(|v| v.parse::<u32>().ok()) {
                    self.multi_pv = v.clamp(1, 64);
                }
            }
            "ponder" => {
                if let Some(v) = value {
                    self.ponder = matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1");
                }
            }
            // Tunable search parameters for SPSA
            "rfpmargin" => {
                if let Some(v) = value.and_then(|v| v.parse::<i32>().ok()) {
                    state.params_mut().rfp_margin = v.clamp(50, 300);
                }
            }
            "nullmovereduction" => {
                if let Some(v) = value.and_then(|v| v.parse::<u32>().ok()) {
                    state.params_mut().null_reduction = v.clamp(1, 5);
                }
            }
            "futilitymargin" => {
                if let Some(v) = value.and_then(|v| v.parse::<i32>().ok()) {
                    state.params_mut().futility_margin = v.clamp(50, 250);
                }
            }
            "iirmindepth" => {
                if let Some(v) = value.and_then(|v| v.parse::<u32>().ok()) {
                    state.params_mut().iir_min_depth = v.clamp(3, 8);
                }
            }
            "lmrmindepth" => {
                if let Some(v) = value.and_then(|v| v.parse::<u32>().ok()) {
                    state.params_mut().lmr_min_depth = v.clamp(2, 6);
                }
            }
            _ => {}
        }
        None
    }
}

#[must_use]
pub fn parse_setoption(parts: &[&str]) -> Option<(String, Option<String>)> {
    if parts.is_empty() || parts[0] != "setoption" {
        return None;
    }

    let mut name_parts: Vec<&str> = Vec::new();
    let mut value_parts: Vec<&str> = Vec::new();
    let mut mode = "";

    for part in parts.iter().skip(1) {
        match *part {
            "name" => mode = "name",
            "value" => mode = "value",
            _ => match mode {
                "name" => name_parts.push(part),
                "value" => value_parts.push(part),
                _ => {}
            },
        }
    }

    if name_parts.is_empty() {
        return None;
    }

    let name = name_parts.join(" ");
    let value = if value_parts.is_empty() {
        None
    } else {
        Some(value_parts.join(" "))
    };

    Some((name, value))
}
