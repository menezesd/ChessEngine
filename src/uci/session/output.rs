use std::sync::Arc;

use crate::board::SearchIterationInfo;

fn print_uci_info(info: &SearchIterationInfo) {
    if let Some(mate) = info.mate_in {
        println!(
            "info depth {} seldepth {} multipv {} nodes {} nps {} time {} score mate {} pv {}",
            info.depth,
            info.seldepth,
            info.multipv,
            info.nodes,
            info.nps,
            info.time_ms,
            mate,
            info.pv
        );
    } else {
        println!(
            "info depth {} seldepth {} multipv {} nodes {} nps {} time {} score cp {} pv {}",
            info.depth,
            info.seldepth,
            info.multipv,
            info.nodes,
            info.nps,
            info.time_ms,
            info.score,
            info.pv
        );
    }
}

pub(super) fn default_info_callback() -> Arc<dyn Fn(&SearchIterationInfo) + Send + Sync> {
    Arc::new(print_uci_info)
}
