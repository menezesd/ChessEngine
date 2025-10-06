use std::sync::mpsc::{Sender, Receiver};

#[derive(Clone, Debug)]
pub struct Info {
    pub depth: Option<u32>,
    pub nodes: Option<u64>,
    pub nps: Option<u64>,
    pub time_ms: Option<u128>,
    pub score_cp: Option<i32>,
    pub score_mate: Option<i32>,
    pub pv: Option<String>,
    pub seldepth: Option<u32>,
    pub ponder: Option<String>,
}

impl Info {
    pub fn to_uci_line(&self) -> String {
        let mut parts = Vec::new();
        if let Some(d) = self.depth { parts.push(format!("depth {}", d)); }
        if let Some(n) = self.nodes { parts.push(format!("nodes {}", n)); }
        if let Some(nps) = self.nps { parts.push(format!("nps {}", nps)); }
        if let Some(t) = self.time_ms { parts.push(format!("time {}", t)); }
        if let Some(cp) = self.score_cp { parts.push(format!("score cp {}", cp)); }
        if let Some(mate) = self.score_mate { parts.push(format!("score mate {}", mate)); }
        if let Some(ref pv) = self.pv { parts.push(format!("pv {}", pv)); }
        if let Some(sd) = self.seldepth { parts.push(format!("seldepth {}", sd)); }
        if let Some(ref p) = self.ponder { parts.push(format!("ponder {}", p)); }
        if parts.is_empty() { "info".to_string() } else { format!("info {}", parts.join(" ")) }
    }
}

pub fn channel() -> (Sender<Info>, Receiver<Info>) {
    std::sync::mpsc::channel()
}

// Additional plumbing for Info-sender across board and uci
// Spawn printer thread for Info messages
// (No runtime code here — just the Info type and channel helper.)