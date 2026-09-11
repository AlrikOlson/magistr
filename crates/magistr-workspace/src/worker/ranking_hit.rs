use serde::{Deserialize, Serialize};

/// One hit-context handed to a ranking signal; the enum fields travel as the
/// WIT case names ("declaration"…, "source"…) exactly as the retained lane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerRankingHit {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub excerpt: String,
    pub kind_class: String,
    pub file_class: String,
}

impl From<WorkerRankingHit> for crate::sandbox::RankingHit {
    fn from(hit: WorkerRankingHit) -> Self {
        Self {
            file: hit.file,
            line: hit.line,
            column: hit.column,
            excerpt: hit.excerpt,
            kind_class: hit.kind_class,
            file_class: hit.file_class,
        }
    }
}
