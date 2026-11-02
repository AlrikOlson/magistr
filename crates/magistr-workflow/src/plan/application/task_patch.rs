use crate::content::StructuredContent;

use super::WaitingDraft;

/// Typed patch; absent fields preserve the stored value.
#[derive(Debug, Clone, Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub name: Option<String>,
    pub priority: Option<u32>,
    pub description: Option<String>,
    pub acceptance: Option<Vec<String>>,
    pub deps: Option<Vec<String>>,
    pub content: Option<StructuredContent>,
    pub resolve_title_proposal: Option<bool>,
    pub waiting_on: Option<WaitingDraft>,
    pub clear_waiting_on: bool,
}
