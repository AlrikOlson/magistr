//! Deterministic, presentation-only data for comparing desktop foundations.
mod workload;
pub use workload::{Event, Fixture, StreamCancellation, StreamRun, StreamState, timestamp_us};
