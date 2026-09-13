use crate::agent;
use crate::error::ResultExt;
use crate::utils::format as fmt;
use crate::workflow;
use color_eyre::eyre;

pub async fn cmd_next_continue(
    dir: &std::path::Path,
    backend: agent::Backend,
    profile: crate::permissions::PermissionProfile,
) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;

    let wf = workflow::WorkflowConfig::load(&dir).suggest()?;

    // Look for an active plan to resume
    let active = workflow::plans::find_active_plan(&dir).suggest()?;

    let plan = match active {
        Some(p) => {
            eprintln!(
                "{} Resuming plan: {}",
                fmt::cyan("→"),
                fmt::bold(&p.task_name)
            );
            eprintln!("  {}", fmt::dim(&format!("Plan ID: {}", p.id)));
            p
        }
        None => {
            eprintln!(
                "{}",
                fmt::warning("No active plan found. Run `magistr next` to create one.")
            );
            return Ok(());
        }
    };

    let summary = wf.spec_summary().suggest()?;
    eprintln!(
        "{}",
        fmt::info(&format!("Spec progress: {}", summary.status_line()))
    );

    if summary.all_done() {
        // Mark the plan as completed
        workflow::plans::update_status(&dir, &plan.id, workflow::plans::PlanStatus::Completed)
            .suggest()?;
        eprintln!(
            "{}",
            fmt::success("All spec items complete. Plan marked as completed.")
        );
        return Ok(());
    }

    // Build a prompt that includes the existing plan as context
    let mut prompt = wf.build_prompt().suggest()?;
    prompt.push_str("\n\n## Existing Plan (resume from here)\n\n");
    prompt.push_str(
        "A previous plan was created for this work. Resume execution from where it left off:\n\n",
    );
    prompt.push_str(&plan.content);

    eprintln!(
        "{}\n",
        fmt::info("Continuing research-plan-execute-verify cycle...")
    );

    let workspace = crate::commands::run::admit_workspace(&dir, |line| eprintln!("{line}")).await;
    let ran = crate::commands::run::cmd_run_workflow(
        &prompt,
        &dir,
        backend,
        profile,
        workspace.source_route(),
        |_| Ok(()),
    )
    .await;
    if let Err(err) = workspace.shutdown().await {
        eprintln!("magistr: workspace shutdown: {err}");
    }
    ran
}
