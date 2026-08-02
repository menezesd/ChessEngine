use super::{TimeConfig, TimeControl};

/// Parameters for executing a search (shared builder for protocol layers).
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub soft_time_ms: u64,
    pub hard_time_ms: u64,
    pub max_nodes: u64,
    pub depth: Option<u32>,
    pub ponder: bool,
    pub infinite: bool,
}

/// Build a search request from a time control and constraints.
#[must_use]
pub fn build_search_request(
    time_control: TimeControl,
    depth: Option<u32>,
    nodes: Option<u64>,
    ponder: bool,
    infinite: bool,
    config: &TimeConfig,
) -> (SearchRequest, (u64, u64)) {
    let planned_limits = if infinite {
        (u64::MAX, u64::MAX)
    } else {
        time_control.compute_limits(config)
    };
    let (soft_ms, hard_ms) = planned_limits;
    let has_active_time_limits = !infinite && !time_control.is_unlimited();

    // `go infinite` must run until `stop`: never inherit the default
    // node budget, which would end the search and emit an unsolicited
    // `bestmove`.
    let max_nodes = if infinite {
        0
    } else {
        nodes.unwrap_or(config.default_max_nodes)
    };

    (
        SearchRequest {
            soft_time_ms: if has_active_time_limits { soft_ms } else { 0 },
            hard_time_ms: if has_active_time_limits { hard_ms } else { 0 },
            max_nodes,
            depth,
            ponder,
            infinite,
        },
        planned_limits,
    )
}

#[cfg(test)]
mod tests;
