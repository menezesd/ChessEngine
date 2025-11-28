use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// Cooperative search control: stop flag and optional node limit + counter
static STOP_FLAG: AtomicBool = AtomicBool::new(false);
static NODE_LIMIT: AtomicU64 = AtomicU64::new(0); // 0 == no limit
static NODE_COUNT: AtomicU64 = AtomicU64::new(0);
// Track maximum ply reached for debugging recursion issues.
static MAX_PLY_OBSERVED: AtomicU64 = AtomicU64::new(0);
static MAX_PLY_LOGGED: AtomicBool = AtomicBool::new(false);
// Optional wall-clock deadline in milliseconds since UNIX_EPOCH; 0 means no deadline.
static DEADLINE_MS: AtomicU64 = AtomicU64::new(0);
// Debug flag to log deadline hits only once.
static DEADLINE_LOGGED: AtomicBool = AtomicBool::new(false);

pub fn reset() {
    STOP_FLAG.store(false, Ordering::SeqCst);
    NODE_LIMIT.store(0, Ordering::SeqCst);
    NODE_COUNT.store(0, Ordering::SeqCst);
    MAX_PLY_OBSERVED.store(0, Ordering::SeqCst);
    MAX_PLY_LOGGED.store(false, Ordering::SeqCst);
    DEADLINE_MS.store(0, Ordering::SeqCst);
    DEADLINE_LOGGED.store(false, Ordering::SeqCst);
}

pub fn set_stop(v: bool) {
    STOP_FLAG.store(v, Ordering::SeqCst);
}

pub fn should_stop() -> bool {
    if STOP_FLAG.load(Ordering::SeqCst) {
        return true;
    }
    let deadline = DEADLINE_MS.load(Ordering::SeqCst);
    if deadline > 0 {
        if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
            if now.as_millis() as u64 >= deadline {
                if cfg!(debug_assertions) && !DEADLINE_LOGGED.swap(true, Ordering::SeqCst) {
                    
                }
                return true;
            }
        }
    }
    false
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

pub fn update_max_ply(ply: usize) {
    let mut current = MAX_PLY_OBSERVED.load(Ordering::SeqCst);
    while ply as u64 > current {
        match MAX_PLY_OBSERVED.compare_exchange(current, ply as u64, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => {
                if cfg!(debug_assertions) && ply > 20 && !MAX_PLY_LOGGED.swap(true, Ordering::SeqCst) {
                    let _ = std::fs::write("trace.txt", format!("max ply observed {}\n", ply));
                }
                break;
            }
            Err(v) => current = v,
        }
    }
}

pub fn get_max_ply() -> u64 {
    MAX_PLY_OBSERVED.load(Ordering::SeqCst)
}

pub fn set_deadline(deadline_ms_since_epoch: u64) {
    DEADLINE_MS.store(deadline_ms_since_epoch, Ordering::SeqCst);
}

pub fn clear_deadline() {
    DEADLINE_MS.store(0, Ordering::SeqCst);
}
