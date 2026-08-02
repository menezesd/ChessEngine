use parking_lot::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::constants::DEFAULT_MAX_DEPTH;

pub(super) fn duration_millis_saturating(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn deadline_remaining_ms(deadline: Instant, now: Instant) -> u64 {
    duration_millis_saturating(deadline.saturating_duration_since(now))
}

/// Result of a search containing best move and ponder move
#[derive(Debug, Clone, Copy)]
pub struct SearchResult {
    /// The best move found
    pub best_move: Option<crate::board::Move>,
    /// The expected opponent reply (for pondering)
    pub ponder_move: Option<crate::board::Move>,
}

/// Time limits for a search
pub struct SearchLimits {
    pub clock: Arc<SearchClock>,
    pub stop: Arc<AtomicBool>,
}

/// Clock for tracking search time limits
pub struct SearchClock {
    start_time: Mutex<Instant>,
    soft_deadline: Mutex<Option<Instant>>,
    hard_deadline: Mutex<Option<Instant>>,
}

impl SearchClock {
    #[must_use]
    pub fn new(
        start_time: Instant,
        soft_deadline: Option<Instant>,
        hard_deadline: Option<Instant>,
    ) -> Self {
        SearchClock {
            start_time: Mutex::new(start_time),
            soft_deadline: Mutex::new(soft_deadline),
            hard_deadline: Mutex::new(hard_deadline),
        }
    }

    pub fn reset(
        &self,
        start_time: Instant,
        soft_deadline: Option<Instant>,
        hard_deadline: Option<Instant>,
    ) {
        let mut start = self.start_time.lock();
        *start = start_time;
        let mut soft = self.soft_deadline.lock();
        *soft = soft_deadline;
        let mut hard = self.hard_deadline.lock();
        *hard = hard_deadline;
    }

    pub fn snapshot(&self) -> (Instant, Option<Instant>, Option<Instant>) {
        let start_time = *self.start_time.lock();
        let soft_deadline = *self.soft_deadline.lock();
        let hard_deadline = *self.hard_deadline.lock();
        (start_time, soft_deadline, hard_deadline)
    }
}

/// Configuration for a search operation.
///
/// This struct consolidates all search parameters into a single configuration
/// object, replacing the need for multiple `find_best_move_*` functions.
#[derive(Clone)]
pub struct SearchConfig {
    /// Maximum depth to search (None = unlimited, defaults to 64)
    pub max_depth: Option<u32>,
    /// Soft time limit in milliseconds (0 = unlimited). The search aims to
    /// stop here, but a running iteration may overrun it (bounded by the
    /// hard limit) when the best move is unstable or the score is dropping.
    pub time_limit_ms: u64,
    /// Hard time limit in milliseconds (0 = none). Absolute mid-tree abort
    /// budget; when 0, the soft limit aborts mid-tree as well.
    pub hard_time_limit_ms: u64,
    /// Optional live clock shared with the controller. When present, the
    /// search reads its deadlines instead of the static millisecond limits,
    /// so a `ponderhit` reset takes effect on the running search.
    pub clock: Option<Arc<SearchClock>>,
    /// Node limit (0 = unlimited)
    pub node_limit: u64,
    /// Whether to extract ponder move from TT after search
    pub extract_ponder: bool,
    /// Optional callback for iteration info
    pub info_callback: Option<SearchInfoCallback>,
    /// Number of principal variations to search (1 = normal, >1 = `MultiPV`)
    pub multi_pv: u32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            max_depth: None,
            time_limit_ms: 0,
            hard_time_limit_ms: 0,
            clock: None,
            node_limit: 0,
            extract_ponder: true,
            info_callback: None,
            multi_pv: 1,
        }
    }
}

impl SearchConfig {
    /// Create a depth-limited search config
    #[must_use]
    pub fn depth(max_depth: u32) -> Self {
        SearchConfig {
            max_depth: Some(max_depth.min(DEFAULT_MAX_DEPTH)),
            ..Default::default()
        }
    }

    /// Create a time-limited search config
    #[must_use]
    pub fn time(time_limit_ms: u64) -> Self {
        SearchConfig {
            time_limit_ms,
            ..Default::default()
        }
    }

    /// Create a config from `SearchLimits`
    #[must_use]
    pub fn from_limits(limits: &SearchLimits) -> Self {
        let (_, soft_deadline, hard_deadline) = limits.clock.snapshot();
        let now = Instant::now();
        let time_limit_ms = soft_deadline.map_or(0, |deadline| deadline_remaining_ms(deadline, now));
        let hard_time_limit_ms =
            hard_deadline.map_or(0, |deadline| deadline_remaining_ms(deadline, now));
        SearchConfig {
            time_limit_ms,
            hard_time_limit_ms,
            clock: Some(Arc::clone(&limits.clock)),
            ..Default::default()
        }
    }

    /// Set whether to extract ponder move
    #[must_use]
    pub fn with_ponder(mut self, extract_ponder: bool) -> Self {
        self.extract_ponder = extract_ponder;
        self
    }

    /// Set node limit
    #[must_use]
    pub fn with_nodes(mut self, node_limit: u64) -> Self {
        self.node_limit = node_limit;
        self
    }

    /// Attach a callback for iteration info reporting.
    #[must_use]
    pub fn with_info_callback(mut self, callback: SearchInfoCallback) -> Self {
        self.info_callback = Some(callback);
        self
    }

    /// Set number of principal variations to search (`MultiPV`)
    #[must_use]
    pub fn with_multi_pv(mut self, multi_pv: u32) -> Self {
        self.multi_pv = multi_pv.max(1);
        self
    }
}

/// Information about a completed search iteration.
#[derive(Debug, Clone)]
pub struct SearchIterationInfo {
    pub depth: u32,
    pub nodes: u64,
    pub nps: u64,
    pub time_ms: u64,
    pub score: i32,
    pub mate_in: Option<i32>,
    pub pv: String,
    pub seldepth: u32,
    pub tt_hits: u64,
    /// Which PV line this is (1 = best, 2 = second best, etc.).
    pub multipv: u32,
}

/// Callback type for iteration info.
pub type SearchInfoCallback = Arc<dyn Fn(&SearchIterationInfo) + Send + Sync>;

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{
        deadline_remaining_ms, duration_millis_saturating, SearchClock, SearchConfig, SearchLimits,
    };
    use crate::board::search::DEFAULT_MAX_DEPTH;

    #[test]
    fn duration_millis_saturates_large_duration() {
        assert_eq!(duration_millis_saturating(Duration::MAX), u64::MAX);
    }

    #[test]
    fn deadline_remaining_ms_returns_time_until_future_deadline() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(250);

        assert_eq!(deadline_remaining_ms(deadline, now), 250);
    }

    #[test]
    fn deadline_remaining_ms_saturates_elapsed_deadline() {
        let now = Instant::now();
        let deadline = now.checked_sub(Duration::from_millis(250)).unwrap();

        assert_eq!(deadline_remaining_ms(deadline, now), 0);
    }

    #[test]
    fn from_limits_uses_zero_when_soft_deadline_is_absent() {
        let now = Instant::now();
        let limits = SearchLimits {
            clock: Arc::new(SearchClock::new(now, None, None)),
            stop: Arc::new(AtomicBool::new(false)),
        };

        assert_eq!(SearchConfig::from_limits(&limits).time_limit_ms, 0);
    }

    #[test]
    fn depth_clamps_to_default_max_depth() {
        assert_eq!(
            SearchConfig::depth(u32::MAX).max_depth,
            Some(DEFAULT_MAX_DEPTH)
        );
    }
}
