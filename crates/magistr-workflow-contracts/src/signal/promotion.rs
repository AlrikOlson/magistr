use crate::{Chunk, Observed, Signal};

/// Each save carries its own committed source; the two owners are not one transaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignalPromotion {
    pub signal_id: String,
    pub chunk_id: String,
    pub created: bool,
    pub signal: Observed<Signal>,
    pub chunk: Observed<Chunk>,
}
