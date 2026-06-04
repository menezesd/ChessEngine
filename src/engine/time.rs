//! Unified time management for UCI and `XBoard` protocols.
//!
//! This module provides a protocol-agnostic time control abstraction that both
//! UCI and `XBoard` handlers can use to compute search time limits.

mod control;
mod limits;
mod request;

pub use control::{TimeConfig, TimeControl, DEFAULT_MOVES_TO_GO};
pub use limits::compute_time_limits;
pub use request::{build_search_request, SearchRequest};
