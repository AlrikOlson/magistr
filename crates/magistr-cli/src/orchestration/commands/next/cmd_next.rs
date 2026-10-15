use crate::agent;
use crate::error::ResultExt;
use crate::utils::format as fmt;
use crate::workflow;
use color_eyre::eyre;

pub async fn cmd_next(
    dir: &std::path::Path,
    backend: agent::Backend,
    profile: crate::permissions::PermissionProfile,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;
    let work_lease = magistr_core::workflow::loop_lease::LoopLease::acquire(&dir).suggest()?;
    work_lease.require_settled(&dir).suggest()?;
    let inputs = super::workflow_inputs(&inputs.for_project(&dir), false)?;

    let wf = workflow::WorkflowConfig::load(&dir).suggest()?;
    let summary = wf.spec_summary().suggest()?;

    eprintln!(
        "{}",
        fmt::info(&format!("Spec progress: {}", summary.status_line()))
    );

    if summary.all_done() {
        eprintln!(
            "{}",
            fmt::success("All spec items complete. Nothing to do.")
        );
        return Ok(());
    }

    let prompt = wf.build_prompt().suggest()?;
    eprintln!(
        "{}\n",
        fmt::info("Starting research-plan-execute-verify cycle...")
    );

    // Delegate to the workflow session with this run's workspace admission.
    let workspace =
        crate::commands::run::admit_workspace(&inputs, |line| eprintln!("{line}")).await;
    // One turn with no loop checkpoint: nothing records the runtime.
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
