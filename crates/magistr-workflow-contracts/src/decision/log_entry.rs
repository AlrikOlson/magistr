//! In-memory log bookkeeping.

use super::log::Log;

/// Log entry tracks the per-log history with a last-access timestamp.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub history: Log,
    /// Unix millis of last access.
    pub last_accessed: u128,
}
