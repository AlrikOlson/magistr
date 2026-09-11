use crate::contracts::Freshness;
/// In-memory progress is reset on restart; persisted cache is revalidated.
#[derive(Debug, Clone)]
pub struct IndexStatus {
    pub freshness: Freshness,
    pub building: bool,
    pub generation: u64,
    pub files: usize,
    pub parsed: usize,
    pub reused: usize,
    pub last_error: Option<String>,
    pub notes: Vec<String>,
}
impl Default for IndexStatus {
    fn default() -> Self {
        Self {
            freshness: Freshness::Cold,
            building: false,
            generation: 0,
            files: 0,
            parsed: 0,
            reused: 0,
            last_error: None,
            notes: vec![
                "Index generation is cache progress, not a source revision or completion evidence."
                    .into(),
            ],
        }
    }
}
