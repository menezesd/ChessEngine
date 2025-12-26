use crate::board::{SearchParams, SearchState, DEFAULT_TT_MB};

pub enum UciOptionAction {
    ReinitHash(usize),
}

pub struct UciOptions {
    pub hash_mb: usize,
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
            default_max_nodes: 0,
            move_overhead_ms: 50,
            soft_time_percent: 70,
            hard_time_percent: 90,
            multi_pv: 1,
            ponder: false,
        }
    }

    pub fn print(&self, _params: &SearchParams) {
        println!("id name chess_engine");
        println!("id author Dean Menezes");
        println!(
            "option name Hash type spin default {} min 1 max 65536",
            self.hash_mb
        );
        println!(
            "option name Move Overhead type spin default {} min 0 max 1000",
            self.move_overhead_ms
        );
        println!(
            "option name Soft Time Percent type spin default {} min 1 max 100",
            self.soft_time_percent
        );
        println!(
            "option name Hard Time Percent type spin default {} min 1 max 100",
            self.hard_time_percent
        );
        println!(
            "option name Max Nodes type spin default {} min 0 max 18446744073709551615",
            self.default_max_nodes
        );
        println!(
            "option name MultiPV type spin default {} min 1 max 64",
            self.multi_pv
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
        _state: &mut SearchState,
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
