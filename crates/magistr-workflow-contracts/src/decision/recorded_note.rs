//! Native result of a committed reasoning mutation.
use crate::ReadSource;
use crate::decision::Note;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordedNote {
    pub project_id: String,
    pub note: Note,
    pub source: ReadSource,
    /// Bounded display projection of the committed state.
    pub report: serde_json::Value,
}
