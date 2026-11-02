use crate::content::StructuredContent;
use crate::plan::domain::TaskStatus;

/// Typed input for creating a native plan task.
#[derive(Debug, Clone)]
pub struct TaskDraft {
    pub id: String,
    pub title: String,
    pub name: String,
    pub status: TaskStatus,
    pub priority: u32,
    pub description: String,
    pub acceptance: Vec<String>,
    pub deps: Vec<String>,
    pub shared: bool,
    pub feature: Option<String>,
    pub content: Option<StructuredContent>,
}
