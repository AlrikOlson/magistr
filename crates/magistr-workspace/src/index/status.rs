use crate::contracts::Freshness;
/// In-memory progress is reset on restart; persisted cache is revalidated.
#[derive(Debug, Clone)]
pub struct IndexStatus {
    pub freshness: Freshness,
    pub building: bool,
    /// A requested refresh or observed file change has not finished scanning.
    pub refresh_pending: bool,
    pub generation: u64,
    pub files: usize,
    pub parsed: usize,
    pub reused: usize,
    pub last_error: Option<String>,
    /// Milliseconds since the Unix epoch when the current or last build
    /// started; `None` until a build has been kicked in this process.
    pub build_started_at_ms: Option<u64>,
    /// Milliseconds since the Unix epoch of the last build status change.
    pub build_updated_at_ms: Option<u64>,
    pub notes: Vec<String>,
}
impl Default for IndexStatus {
    fn default() -> Self {
        Self {
            freshness: Freshness::Cold,
            building: false,
            refresh_pending: false,
            generation: 0,
            files: 0,
            parsed: 0,
            reused: 0,
            last_error: None,
            build_started_at_ms: None,
            build_updated_at_ms: None,
            notes: vec![
                "Index generation is cache progress, not a source revision or completion evidence."
                    .into(),
            ],
        }
    }
}
