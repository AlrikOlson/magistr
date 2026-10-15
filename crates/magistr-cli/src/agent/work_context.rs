//! Current work is read once from the addressed workflow owner at launch.

use super::AgentConfig;
use crate::error::{MagistrError, Result};

pub(super) fn apply(config: &mut AgentConfig) -> Result<()> {
    let mut native = config
        .inputs()
        .workflow()
        .clone()
        .with_project_root(&config.cwd);
    if let Some(execution) = &config.execution_id {
        native = native.with_execution_id(execution);
    }
    config.execution_id = Some(native.execution_context().execution_id.clone());
    let reader = magistr_engine::NativeReader::open(&native);
    let work = reader.ship_snapshot().map_err(|error| {
        MagistrError::Workflow(format!("Could not read the current work: {error}"))
    })?;
    let current = work.current.map(|run| {
        let tasks: Vec<_> = run
            .tasks
            .iter()
            .map(|task| {
                serde_json::json!({
                    "id": task.id,
                    "instance": run.task_instances.get(&task.id),
                    "title": task.title,
                    "status": task.status,
                    "type": task.task_type,
                    "blocked_reason": task.blocked_reason,
                })
            })
            .collect();
        serde_json::json!({
            "run_id": run.run_id,
            "objective": run.objective,
            "tasks": tasks,
            "active_task": run.tasks.iter()
                .find(|task| task.status == magistr_engine::native::TaskStatus::Active)
                .map(|task| &task.id),
        })
    });
    let context = format!(
        "## Current work\n\nThis is a launch-time observation of this session's work. Read ship_status before resuming after context compaction or acting on a plan that may have changed. A missing objective means no work has been selected. A completed objective does not authorize starting new work. Follow the user's request and this conversation's purpose.\n\n{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "working_copy": native.execution_context().working_copy,
            "execution": native.execution_context().execution_id,
            "purpose": match config.history.purpose() {
                magistr_application::sessions::SessionPurpose::Conversation => "conversation",
                magistr_application::sessions::SessionPurpose::Roadmap => "planning",
            },
            "current": current,
        }))
        .map_err(|error| MagistrError::Workflow(error.to_string()))?
    );
    config.system_prompt = Some(match config.system_prompt.take() {
        Some(prompt) => format!("{prompt}\n\n{context}"),
        None => context,
    });
    Ok(())
}
