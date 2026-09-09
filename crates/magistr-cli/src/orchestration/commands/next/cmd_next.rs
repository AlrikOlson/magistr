use crate::agent;
use crate::error::ResultExt;
use crate::utils::format as fmt;
use crate::workflow;
use color_eyre::eyre;

pub async fn cmd_next(dir: &std::path::Path, backend: agent::Backend) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;

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

    // Clean up leftover state from previous runs
    wf.cleanup().suggest()?;

    let prompt = wf.build_prompt().suggest()?;
    eprintln!(
        "{}\n",
        fmt::info("Starting research-plan-execute-verify cycle...")
    );

    // Delegate to cmd_run with the workflow prompt
    crate::commands::run::cmd_run(&prompt, None, false, None, backend).await
}
