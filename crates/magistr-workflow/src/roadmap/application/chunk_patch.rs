use crate::content::StructuredContent;

use super::BlockerDraft;

/// Typed patch; absent fields preserve the stored value.
#[derive(Debug, Clone, Default)]
pub struct ChunkPatch {
    pub title: Option<String>,
    pub name: Option<String>,
    pub priority: Option<u32>,
    pub description: Option<String>,
    pub acceptance: Option<Vec<String>>,
    pub deps: Option<Vec<String>>,
    pub content: Option<StructuredContent>,
    pub resolve_title_proposal: Option<bool>,
    pub blocked_by: Option<BlockerDraft>,
    pub clear_blocked_by: bool,
}
