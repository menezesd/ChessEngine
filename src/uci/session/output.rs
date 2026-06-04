use std::sync::Arc;

use crate::board::SearchIterationInfo;

fn print_uci_info(info: &SearchIterationInfo) {
    let multipv_str = if info.multipv > 1 {
        format!(" multipv {}", info.multipv)
    } else {
        String::new()
    };

    if let Some(mate) = info.mate_in {
        println!(
            "info depth {} seldepth {}{} nodes {} nps {} time {} score mate {} pv {}",
            info.depth,
            info.seldepth,
            multipv_str,
            info.nodes,
            info.nps,
            info.time_ms,
            mate,
            info.pv
        );
    } else {
        println!(
            "info depth {} seldepth {}{} nodes {} nps {} time {} score cp {} pv {}",
            info.depth,
            info.seldepth,
            multipv_str,
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
