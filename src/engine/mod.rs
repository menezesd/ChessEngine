//! Engine controller for managing search and game state.
//!
//! This module provides a unified interface for both UCI and `XBoard` protocols,
//! abstracting away the common logic of search management, pondering, and
//! time control.

mod controller;
mod protocol;
pub mod time;

pub use controller::{EngineController, SearchJob, SearchParams};
pub use protocol::{CommandResult, Protocol, ProtocolType};
pub use time::{build_search_request, compute_time_limits, TimeConfig, TimeControl};
