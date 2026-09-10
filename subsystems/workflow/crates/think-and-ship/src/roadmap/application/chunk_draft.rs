use crate::content::StructuredContent;
use crate::roadmap::domain::ChunkStatus;

/// Typed input for creating a native roadmap chunk.
#[derive(Debug, Clone)]
pub struct ChunkDraft {
    pub id: String,
    pub title: String,
    pub name: String,
    pub status: ChunkStatus,
    pub priority: u32,
    pub description: String,
    pub acceptance: Vec<String>,
    pub deps: Vec<String>,
    pub shared: bool,
    pub group: Option<String>,
    pub content: Option<StructuredContent>,
}
