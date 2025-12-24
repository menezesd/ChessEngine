pub struct SearchInfo {
    pub depth: u32,
    pub seldepth: u32,
    pub score: String,
    pub nodes: u64,
    pub nps: u64,
    pub hashfull: u32,
    pub time_ms: u128,
    pub pv: String,
}

pub trait SearchLogger {
    fn info(&self, info: &SearchInfo);
}

pub struct StdoutLogger;

impl SearchLogger for StdoutLogger {
    fn info(&self, info: &SearchInfo) {
        println!(
            "info depth {} seldepth {} score {} nodes {} nps {} hashfull {} time {} pv {}",
            info.depth,
            info.seldepth,
            info.score,
            info.nodes,
            info.nps,
            info.hashfull,
            info.time_ms,
            info.pv
        );
    }
}
