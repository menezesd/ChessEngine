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
        "info string time soft {soft_time_ms} hard {hard_time_ms} overhead {move_overhead_ms} nodes {max_nodes} ponder {ponder} depth {depth}"
    );
}
