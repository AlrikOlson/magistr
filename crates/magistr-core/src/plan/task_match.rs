//! Task search result type.

/// Result of a task search.
#[derive(Debug, Clone)]
pub struct TaskMatch {
    pub phase_idx: usize,
    pub task_path: Vec<usize>,
    pub text: String,
    pub checked: bool,
    pub phase_id: String,
    pub phase_name: String,
    /// Stable hierarchical ID: `"{phase_id}.{task_idx}.{subtask_idx}..."`.
    pub task_id: String,
}
