//! Subagent task tracking types.

/// Status of a tracked subagent task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Done,
    Failed,
}

/// A tracked subagent task visible in the UI.
#[derive(Debug, Clone)]
pub struct ActiveTask {
    /// tool_use_id for correlating lifecycle events
    pub id: String,
    /// Short description of the task
    pub description: String,
    /// Current status
    pub status: TaskStatus,
}

#[cfg(test)]
#[path = "task.tests.rs"]
mod tests;
