use crate::agent;
use crate::error::{MagistrError, ResultExt};
use crate::utils::format as fmt;
use crate::workflow;
use color_eyre::eyre;
use magistr_engine::native::ObjectiveStatus;

pub async fn cmd_next_continue(
    dir: &std::path::Path,
    backend: agent::Backend,
    profile: crate::permissions::PermissionProfile,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;
    let work_lease = magistr_core::workflow::loop_lease::LoopLease::acquire(&dir).suggest()?;
    work_lease.require_settled(&dir).suggest()?;
    let inputs = inputs.for_project(&dir);
    let reader = magistr_engine::NativeReader::open(inputs.workflow());
    let work = reader
        .ship_snapshot()
        .map_err(MagistrError::Workflow)
        .suggest()?;
    let Some(plan) = work.current.filter(|run| {
        matches!(
            run.objective.status,
            ObjectiveStatus::Defined | ObjectiveStatus::Active
        )
    }) else {
        eprintln!("{}", fmt::warning("No unfinished work is selected."));
        return Ok(());
    };
    eprintln!(
        "{} Continuing: {}",
        fmt::cyan("→"),
        fmt::bold(&plan.objective.description)
    );
    let inputs = super::workflow_inputs(&inputs, false)?;
    let wf = workflow::WorkflowConfig::load(&dir)
        .suggest()?
        .with_native(Some(reader));
    let summary = wf.spec_summary().suggest()?;
    let mut prompt = workflow::prompt::build_next_prompt(&wf.project_config, &summary);
    prompt.push_str("\n\nContinue this execution's selected unfinished objective and plan. Read ship_status first and retain its existing tasks and evidence. Do not select new work or infer objective completion from overall roadmap counts.");

    let workspace =
        crate::commands::run::admit_workspace(&inputs, |line| eprintln!("{line}")).await;
    let ran = crate::commands::run::cmd_run_workflow(
        &prompt,
        &dir,
        backend,
        profile,
        workspace.source_route(),
        &inputs,
        |_| Ok(()),
    )
    .await;
    if let Err(err) = workspace.shutdown().await {
        eprintln!("magistr: workspace shutdown: {err}");
    }
    ran
}

#[cfg(test)]
#[path = "cmd_next_continue.tests.rs"]
mod tests;
