//! Bounded invalidation hints. Query-time source verification remains authoritative.
//!
//! Status is in memory: this adapter never writes the workspace cache or a status file.
mod change_state;
mod index_watcher;
mod network_detection;
mod polling_digest;
mod watch_mode;
mod watch_snapshot;

pub use index_watcher::IndexWatcher;
pub use network_detection::NetworkDetection;
pub use watch_mode::WatchMode;
pub use watch_snapshot::WatchSnapshot;
