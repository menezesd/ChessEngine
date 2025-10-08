use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// Cooperative search control: stop flag and optional node limit + counter
static STOP_FLAG: AtomicBool = AtomicBool::new(false);
static NODE_LIMIT: AtomicU64 = AtomicU64::new(0); // 0 == no limit
static NODE_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn reset() {
    STOP_FLAG.store(false, Ordering::SeqCst);
    NODE_LIMIT.store(0, Ordering::SeqCst);
    NODE_COUNT.store(0, Ordering::SeqCst);
}

pub fn set_stop(v: bool) {
    STOP_FLAG.store(v, Ordering::SeqCst);
}

pub fn should_stop() -> bool {
    STOP_FLAG.load(Ordering::SeqCst)
}

pub fn set_node_limit(limit: u64) {
    NODE_LIMIT.store(limit, Ordering::SeqCst);
    NODE_COUNT.store(0, Ordering::SeqCst);
}

// Increment node count and return true if limit reached
pub fn node_visited() -> bool {
    let n = NODE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
    let limit = NODE_LIMIT.load(Ordering::SeqCst);
    if limit > 0 && n >= limit {
        STOP_FLAG.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

// Read current node count (for reporting)
pub fn get_node_count() -> u64 {
    NODE_COUNT.load(Ordering::SeqCst)
}
