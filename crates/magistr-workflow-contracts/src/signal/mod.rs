//! Canonical signal records and typed mutation events, without host effects.

mod frame;
mod records;

pub use frame::SignalFrame;
pub use records::{
    Enrichment, Signal, SignalKind, SignalStatus, Signals, merge_signal_stores, signal_wins,
};
