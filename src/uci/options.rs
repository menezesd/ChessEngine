use crate::board::{SearchParams, SearchState};

pub enum UciOptionAction {
    ReinitHash(usize),
}

pub struct UciOptions {
    pub hash_mb: usize,
    pub move_overhead_ms: u64,
    pub soft_time_percent: u64,
    pub hard_time_percent: u64,
    pub default_max_nodes: u64,
    pub ponder: bool,
    pub multipv: u32,
}

impl UciOptions {
    pub fn new(hash_mb: usize) -> Self {
        UciOptions {
            hash_mb,
            move_overhead_ms: 50,
            soft_time_percent: 80,
            hard_time_percent: 95,
            default_max_nodes: 0,
            ponder: false,
            multipv: 1,
        }
    }

    pub fn print(&self, params: &SearchParams) {
        println!("id name MyRustEngine");
        println!("id author Dean Menezes");
        println!("option name Hash type spin default {} min 1 max 32768", self.hash_mb);
        println!("option name Clear Hash type button");
        println!(
            "option name Move Overhead type spin default {} min 0 max 500",
            self.move_overhead_ms
        );
        println!(
            "option name SoftTime type spin default {} min 10 max 100",
            self.soft_time_percent
        );
        println!(
            "option name HardTime type spin default {} min 10 max 100",
            self.hard_time_percent
        );
        println!(
            "option name Nodes type spin default {} min 0 max 1000000000",
            self.default_max_nodes
        );
        println!(
            "option name MultiPV type spin default {} min 1 max 4",
            self.multipv
        );
        println!("option name Null Reduction type spin default {} min 0 max 6", params.null_reduction);
        println!(
            "option name Null Min Depth type spin default {} min 0 max 10",
            params.null_min_depth
        );
        println!(
            "option name Null Verify Depth type spin default {} min 0 max 12",
            params.null_verification_depth
        );
        println!(
            "option name LMR Reduction type spin default {} min 0 max 3",
            params.lmr_reduction
        );
        println!(
            "option name LMR Min Depth type spin default {} min 1 max 10",
            params.lmr_min_depth
        );
        println!(
            "option name LMR Min Move type spin default {} min 1 max 20",
            params.lmr_min_move
        );
        println!(
            "option name LMP Min Depth type spin default {} min 1 max 10",
            params.lmp_min_depth
        );
        println!(
            "option name LMP Move Limit type spin default {} min 1 max 32",
            params.lmp_move_limit
        );
        println!(
            "option name Futility Margin type spin default {} min 0 max 1000",
            params.futility_margin
        );
        println!(
            "option name Razor Margin type spin default {} min 0 max 1000",
            params.razor_margin
        );
        println!(
            "option name RFP Margin type spin default {} min 0 max 1000",
            params.rfp_margin
        );
        println!(
            "option name Static Null Margin type spin default {} min 0 max 1000",
            params.static_null_margin
        );
        println!(
            "option name Delta Margin type spin default {} min 0 max 1000",
            params.delta_margin
        );
        println!(
            "option name IIR Min Depth type spin default {} min 1 max 12",
            params.iir_min_depth
        );
        println!(
            "option name Singular Margin type spin default {} min 0 max 200",
            params.singular_margin
        );
        println!(
            "option name Ponder type check default {}",
            if self.ponder { "true" } else { "false" }
        );
        println!("uciok");
    }

    pub fn apply_setoption(
        &mut self,
        name: &str,
        value: Option<&str>,
        search: &mut SearchState,
    ) -> Option<UciOptionAction> {
        match name {
            "Hash" => {
                if let Some(v) = value.and_then(|v| v.parse::<usize>().ok()) {
                    if v > 0 {
                        self.hash_mb = v;
                        return Some(UciOptionAction::ReinitHash(v));
                    }
                }
            }
            "Clear Hash" => return Some(UciOptionAction::ReinitHash(self.hash_mb)),
            "Move Overhead" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.move_overhead_ms = v;
                }
            }
            "SoftTime" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.soft_time_percent = v.clamp(10, 100);
                }
            }
            "HardTime" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.hard_time_percent = v.clamp(10, 100);
                }
            }
            "Nodes" => {
                if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                    self.default_max_nodes = v;
                }
            }
            "Ponder" => {
                if let Some(v) = value {
                    self.ponder = v == "true";
                }
            }
            "MultiPV" => {
                if let Some(v) = value.and_then(|v| v.parse::<u32>().ok()) {
                    self.multipv = v.max(1);
                }
            }
            _ => {
                if let Some(v) = value {
                    if update_search_params(name, v, search.params_mut()) {
                        return None;
                    }
                }
            }
        }
        None
    }
}

pub fn parse_setoption(parts: &[&str]) -> Option<(String, Option<String>)> {
    let name_idx = parts.iter().position(|p| *p == "name")?;
    let value_idx = parts.iter().position(|p| *p == "value");
    let name = match value_idx {
        Some(v_idx) if v_idx > name_idx + 1 => parts[name_idx + 1..v_idx].join(" "),
        None if name_idx + 1 < parts.len() => parts[name_idx + 1..].join(" "),
        _ => return None,
    };
    let value = value_idx.and_then(|v_idx| {
        if v_idx + 1 < parts.len() {
            Some(parts[v_idx + 1..].join(" "))
        } else {
            None
        }
    });
    Some((name, value))
}

fn update_search_params(name: &str, value: &str, params: &mut SearchParams) -> bool {
    match name {
        "Null Reduction" => {
            if let Ok(v) = value.parse::<u32>() {
                params.null_reduction = v.min(6);
            }
        }
        "Null Min Depth" => {
            if let Ok(v) = value.parse::<u32>() {
                params.null_min_depth = v.min(10);
            }
        }
        "Null Verify Depth" => {
            if let Ok(v) = value.parse::<u32>() {
                params.null_verification_depth = v.min(12);
            }
        }
        "LMR Reduction" => {
            if let Ok(v) = value.parse::<u32>() {
                params.lmr_reduction = v.min(3);
            }
        }
        "LMR Min Depth" => {
            if let Ok(v) = value.parse::<u32>() {
                params.lmr_min_depth = v.max(1).min(10);
            }
        }
        "LMR Min Move" => {
            if let Ok(v) = value.parse::<usize>() {
                params.lmr_min_move = v.max(1).min(20);
            }
        }
        "LMP Min Depth" => {
            if let Ok(v) = value.parse::<u32>() {
                params.lmp_min_depth = v.max(1).min(10);
            }
        }
        "LMP Move Limit" => {
            if let Ok(v) = value.parse::<usize>() {
                params.lmp_move_limit = v.max(1).min(32);
            }
        }
        "Futility Margin" => {
            if let Ok(v) = value.parse::<i32>() {
                params.futility_margin = v.clamp(0, 1000);
            }
        }
        "Razor Margin" => {
            if let Ok(v) = value.parse::<i32>() {
                params.razor_margin = v.clamp(0, 1000);
            }
        }
        "RFP Margin" => {
            if let Ok(v) = value.parse::<i32>() {
                params.rfp_margin = v.clamp(0, 1000);
            }
        }
        "Static Null Margin" => {
            if let Ok(v) = value.parse::<i32>() {
                params.static_null_margin = v.clamp(0, 1000);
            }
        }
        "Delta Margin" => {
            if let Ok(v) = value.parse::<i32>() {
                params.delta_margin = v.clamp(0, 1000);
            }
        }
        "IIR Min Depth" => {
            if let Ok(v) = value.parse::<u32>() {
                params.iir_min_depth = v.max(1).min(12);
            }
        }
        "Singular Margin" => {
            if let Ok(v) = value.parse::<i32>() {
                params.singular_margin = v.clamp(0, 200);
            }
        }
        _ => return false,
    }
    true
}
