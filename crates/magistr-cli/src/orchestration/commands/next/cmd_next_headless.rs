use std::io::Write;
use std::time::Instant;

use color_eyre::eyre;

use crate::agent;
use crate::cli::HeadlessFormat;
use crate::commands::run::{HeadlessOutcome, drain_headless};
use crate::error::ResultExt;
use crate::workflow;

/// Run a single `magistr next` iteration in headless mode.
///
/// Streams agent output to stdout (plain or NDJSON), then emits an
/// iteration report with status, spec progress, and duration.
/// Applies the shared project policy; unanswered approvals are refused by default.
pub async fn cmd_next_headless(
    dir: &std::path::Path,
    backend: agent::Backend,
    format: HeadlessFormat,
    profile: crate::permissions::PermissionProfile,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;
    let inputs = super::workflow_inputs(&inputs.for_project(&dir), false)?;

    let wf = workflow::WorkflowConfig::load(&dir).suggest()?;
    let summary_before = wf.spec_summary().suggest()?;

    if summary_before.all_done() {
        emit_complete(&format)?;
        return Ok(());
    }

    wf.cleanup().suggest()?;
    let prompt = wf.build_prompt().suggest()?;

    let config = workflow_agent_config(&prompt, &dir, backend)
        .with_inputs(&inputs)
        .permission_profile(profile);

    let workspace =
        crate::commands::run::admit_workspace(&inputs, |line| eprintln!("{line}")).await;
    let mut session = agent::session::AgentSession::start(config, workspace.source_route())
        .await
        .suggest()?;

    let budget_limit = inputs.budget_limit();

    // Fire OnIterationStart lifecycle hooks
    crate::workflow::hook_dispatch::run_hooks_headless(
        &dir,
        &wf.project_config,
        &magistr_engine::LifecycleEvent::OnIterationStart,
    )
    .await;

    let start = Instant::now();
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();

    let mut spend = None;
    let drained = drain_headless(
        &mut session,
        format,
        budget_limit,
        &mut stdout,
        &mut stderr,
        &mut spend,
    )
    .await;
    let cleanup = session.terminate_session().await.suggest();
    drop(session);
    if let Err(err) = workspace.shutdown().await {
        eprintln!("magistr: workspace shutdown: {err}");
    }
    let outcome = drained?;
    if let Some(spend) = spend {
        crate::cost::record_session(&inputs, &dir, &spend);
    }
    cleanup?;

    let duration = start.elapsed();

    // Fire OnIterationEnd lifecycle hooks
    crate::workflow::hook_dispatch::run_hooks_headless(
        &dir,
        &wf.project_config,
        &magistr_engine::LifecycleEvent::OnIterationEnd,
    )
    .await;

    // Re-read spec to get post-iteration state
    let summary_after = wf.spec_summary().unwrap_or_else(|_| summary_before.clone());

    // Emit iteration report
    emit_report(
        &format,
        &outcome,
        &summary_before,
        &summary_after,
        duration,
        &mut stdout,
    )?;

    let code = outcome.exit_code();
    if code != 0 {
        std::process::exit(code);
    }

    Ok(())
}

/// Bind both session execution and injected context to the selected project.
pub(super) fn workflow_agent_config(
    prompt: &str,
    dir: &std::path::Path,
    backend: agent::Backend,
) -> agent::AgentConfig {
    agent::AgentConfig::new(prompt, dir)
        .backend(backend)
        .workflow_permissions(true)
        .allowed_tools(agent::ToolPermissions::for_workflow().into_strings())
        .with_project_context(dir)
}

/// Emit a "spec complete" message and exit.
fn emit_complete(format: &HeadlessFormat) -> eyre::Result<()> {
    let mut stdout = std::io::stdout().lock();
    match format {
        HeadlessFormat::Plain => {
            writeln!(stdout, "All spec items complete. Nothing to do.")?;
        }
        HeadlessFormat::Json => {
            let report = serde_json::json!({
                "type": "iteration_report",
                "status": "spec_complete",
            });
            serde_json::to_writer(&mut stdout, &report)?;
            writeln!(stdout)?;
        }
    }
    Ok(())
}

/// Emit the final iteration report.
fn emit_report(
    format: &HeadlessFormat,
    outcome: &HeadlessOutcome,
    before: &workflow::spec::SpecSummary,
    after: &workflow::spec::SpecSummary,
    duration: std::time::Duration,
    w: &mut dyn Write,
) -> std::io::Result<()> {
    let status = match outcome {
        HeadlessOutcome::Ok => "success",
        HeadlessOutcome::AgentError => "agent_error",
        HeadlessOutcome::Cancelled => "cancelled",
        HeadlessOutcome::BudgetExceeded => "budget_exceeded",
    };

    match format {
        HeadlessFormat::Plain => {
            writeln!(w)?;
            writeln!(w, "--- iteration report ---")?;
            writeln!(w, "status: {status}")?;
            writeln!(
                w,
                "spec: {}/{} -> {}/{}",
                before.checked, before.total, after.checked, after.total
            )?;
            writeln!(w, "duration: {:.1}s", duration.as_secs_f64())?;
        }
        HeadlessFormat::Json => {
            let report = serde_json::json!({
                "type": "iteration_report",
                "status": status,
                "spec_before": {
                    "checked": before.checked,
                    "total": before.total,
                },
                "spec_after": {
                    "checked": after.checked,
                    "total": after.total,
                },
                "duration_secs": duration.as_secs_f64(),
            });
            serde_json::to_writer(&mut *w, &report)?;
            writeln!(w)?;
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "cmd_next_headless.tests.rs"]
mod tests;
