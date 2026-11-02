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
    let work = reader.run_snapshot().map_err(|error| {
        MagistrError::Workflow(format!("Could not read the current work: {error}"))
    })?;
    let current = work.current.map(|run| {
        let steps: Vec<_> = run
            .steps
            .iter()
            .map(|step| {
                serde_json::json!({
                    "id": step.id,
                    "instance": run.step_instances.get(&step.id),
                    "title": step.title,
                    "status": step.status,
                    "type": step.step_type,
                    "waiting_reason": step.waiting_reason,
                })
            })
            .collect();
        serde_json::json!({
            "run_id": run.run_id,
            "goal": run.goal,
            "steps": steps,
            "active_step": run.steps.iter()
                .find(|step| step.status == magistr_engine::native::StepStatus::Active)
                .map(|step| &step.id),
        })
    });
    let context = format!(
        "## Current work\n\nThis is a launch-time observation of this session's work. Read run_status before resuming after context compaction or acting on a plan that may have changed. A missing goal means no work has been selected. A completed goal does not authorize starting new work. Follow the user's request and this conversation's purpose.\n\n{}",
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
