//! Plan file lifecycle management for the workflow engine.
//!
//! Manages `.magistr/current-plan.md` and `.magistr/current-task.md` state files,
//! as well as the next-loop iteration state file.

use std::path::{Path, PathBuf};

use crate::error::{MagistrError, Result};

/// Path to the current plan file (`.magistr/current-plan.md`) within a project.
pub fn plan_path(project_dir: &Path) -> PathBuf {
    crate::paths::plan(project_dir)
}

/// Path to the current task name file (`.magistr/current-task.md`) within a project.
pub fn task_path(project_dir: &Path) -> PathBuf {
    crate::paths::task(project_dir)
}

/// Path to the iteration log file (`.magistr/iteration-log.md`) within a project.
pub fn iteration_log_path(project_dir: &Path) -> PathBuf {
    crate::paths::iteration_log(project_dir)
}

/// Path to the next-loop state file.
pub fn loop_state_path(state_prefix: &str) -> PathBuf {
    crate::paths::loop_state(state_prefix)
}

/// Check if a plan file exists.
pub fn plan_exists(project_dir: &Path) -> bool {
    plan_path(project_dir).is_file()
}

/// Read the current plan file content.
pub fn read_plan(project_dir: &Path) -> Result<String> {
    let path = plan_path(project_dir);
    std::fs::read_to_string(&path).map_err(|e| MagistrError::io(&path, e))
}

/// Delete plan and task state files (cleanup after successful iteration).
pub fn cleanup_state(project_dir: &Path) -> Result<()> {
    let plan = plan_path(project_dir);
    let task = task_path(project_dir);

    if plan.exists() {
        std::fs::remove_file(&plan).map_err(|e| MagistrError::io(&plan, e))?;
    }
    if task.exists() {
        std::fs::remove_file(&task).map_err(|e| MagistrError::io(&task, e))?;
    }
    Ok(())
}

/// Next-loop iteration state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopState {
    /// Current iteration number (0-based).
    pub iteration: u32,
    /// Status of the last iteration.
    pub last_status: String,
}

impl Default for LoopState {
    fn default() -> Self {
        Self {
            iteration: 0,
            last_status: "none".to_string(),
        }
    }
}

impl LoopState {
    /// Create initial loop state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse loop state from the state file content.
    pub fn parse(content: &str) -> Self {
        let parts: Vec<&str> = content.trim().splitn(2, ' ').collect();
        let iteration = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let last_status = parts.get(1).unwrap_or(&"none").to_string();
        Self {
            iteration,
            last_status,
        }
    }

    /// Serialize to string for writing to state file.
    pub fn serialize(&self) -> String {
        format!("{} {}", self.iteration, self.last_status)
    }
}

/// Check if the next-loop is active.
pub fn loop_is_active(state_prefix: &str) -> bool {
    loop_state_path(state_prefix).is_file()
}

/// Read loop state from the state file.
pub fn read_loop_state(state_prefix: &str) -> Result<LoopState> {
    let path = loop_state_path(state_prefix);
    let content = std::fs::read_to_string(&path).map_err(|e| MagistrError::io(&path, e))?;
    Ok(LoopState::parse(&content))
}

/// Write loop state to the state file.
pub fn write_loop_state(state_prefix: &str, state: &LoopState) -> Result<()> {
    let path = loop_state_path(state_prefix);
    std::fs::write(&path, state.serialize()).map_err(|e| MagistrError::io(&path, e))
}

/// Remove the loop state file (stop the loop).
pub fn remove_loop_state(state_prefix: &str) -> Result<()> {
    let path = loop_state_path(state_prefix);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| MagistrError::io(&path, e))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "plan.tests.rs"]
mod tests;
