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
    depth: Option<u32>,
) {
    println!(
        "{}",
        time_info_line(
            soft_time_ms,
            hard_time_ms,
            move_overhead_ms,
            max_nodes,
            ponder,
            depth
        )
    );
}

fn time_info_line(
    soft_time_ms: u64,
    hard_time_ms: u64,
    move_overhead_ms: u64,
    max_nodes: u64,
    ponder: bool,
    depth: Option<u32>,
) -> String {
    let depth = depth.unwrap_or(0);
    format!(
        "info string time soft {soft_time_ms} hard {hard_time_ms} overhead {move_overhead_ms} nodes {max_nodes} ponder {ponder} depth {depth}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_info_line_formats_depth() {
        let line = time_info_line(100, 200, 10, 1_000, false, Some(12));
        assert_eq!(
            line,
            "info string time soft 100 hard 200 overhead 10 nodes 1000 ponder false depth 12"
        );
    }

    #[test]
    fn time_info_line_defaults_missing_depth_to_zero() {
        let line = time_info_line(100, 200, 10, 1_000, true, None);
        assert_eq!(
            line,
            "info string time soft 100 hard 200 overhead 10 nodes 1000 ponder true depth 0"
        );
    }
}
