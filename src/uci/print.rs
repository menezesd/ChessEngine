use std::time::Duration;

pub fn format_time_setting(value_ms: u64) -> String {
    if value_ms == u64::MAX {
        "inf".to_string()
    } else {
        value_ms.to_string()
    }
}

pub fn print_time_info(
    soft_time_ms: u64,
    hard_time_ms: u64,
    overhead_ms: u64,
    nodes: u64,
    ponder: bool,
    depth: u32,
) {
    println!(
        "info string time soft={} hard={} overhead={} nodes={} ponder={} depth={}",
        format_time_setting(soft_time_ms),
        format_time_setting(hard_time_ms),
        overhead_ms,
        nodes,
        ponder,
        depth
    );
}

pub fn print_perft_info(depth: usize, nodes: u64, elapsed: Duration) {
    println!(
        "info string perft depth {} nodes {} time {:?}",
        depth, nodes, elapsed
    );
}
