use std::time::Duration;

pub fn print_perft_info(depth: usize, nodes: u64, elapsed: Duration) {
    println!(
        "info string perft depth {} nodes {} time_ms {}",
        depth,
        nodes,
        elapsed.as_millis()
    );
}

pub fn print_time_info(
    soft_time_ms: u64,
    hard_time_ms: u64,
    move_overhead_ms: u64,
    max_nodes: u64,
    ponder: bool,
    depth: u32,
) {
    println!(
        "info string time soft {} hard {} overhead {} nodes {} ponder {} depth {}",
        soft_time_ms, hard_time_ms, move_overhead_ms, max_nodes, ponder, depth
    );
}
