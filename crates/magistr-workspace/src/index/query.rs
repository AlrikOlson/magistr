use crate::contracts::Freshness;
/// Bounded candidate results, observed against current file bytes and modes.
/// Generation identifies cache progress only, never a source revision.
#[derive(Debug)]
pub struct IndexQuery<T> {
    pub rows: Vec<T>,
    pub freshness: Freshness,
    pub generation: u64,
    pub truncated: bool,
    pub notes: Vec<String>,
}
