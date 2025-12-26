//! UCI time management.
//!
//! Re-exports the unified time management from engine module for backward compatibility.

// Re-export from the unified engine::time module
pub use crate::engine::time::{compute_time_limits, TimeControl, DEFAULT_MOVES_TO_GO};
