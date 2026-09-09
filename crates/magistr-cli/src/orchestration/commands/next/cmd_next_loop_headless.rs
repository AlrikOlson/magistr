use std::io::Write;
use std::time::Instant;

use color_eyre::eyre;

use crate::agent;
use crate::cli::HeadlessFormat;
use crate::commands::run::{HeadlessOutcome, drain_headless};
use crate::error::ResultExt;
use crate::workflow;
use crate::workflow::iteration::{IterationLoop, PostCheckResult, PreCheckResult};

/// Run `magistr next-loop` in headless mode — batch iterations with progress reporting.
///
/// Streams each iteration's agent output, then emits per-iteration progress as
/// JSON or plain text. Respects `--max-iterations` and `--max-cost` limits.
/// Emits a summary report on completion.
pub async fn cmd_next_loop_headless(
    dir: &std::path::Path,
    max_iterations: u32,
    backend: agent::Backend,
    format: HeadlessFormat,
    max_cost: Option<f64>,
) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;

    let wf = workflow::WorkflowConfig::load(&dir).suggest()?;
    wf.cleanup()?;

    let mut iter_loop = IterationLoop::open(
        &dir,
        &wf.spec_path,
        max_iterations,
        crate::roadmap::open_native_engines(&dir),
    )
    .suggest()?;
    iter_loop.persist().suggest()?;

    let loop_start = Instant::now();
    let mut cumulative_cost = 0.0;
    let mut final_outcome = HeadlessOutcome::Ok;
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();

    loop {
        match iter_loop.pre_iteration_check().suggest()? {
            PreCheckResult::Continue => {}
            PreCheckResult::SpecComplete => {
                emit_loop_stop(&format, "spec_complete", &mut stdout)?;
                break;
            }
            PreCheckResult::MaxIterations => {
                emit_loop_stop(&format, "max_iterations", &mut stdout)?;
                break;
            }
            PreCheckResult::NoReadyChunk => {
                emit_loop_stop(&format, "no_ready_chunk", &mut stdout)?;
                break;
            }
        }

        // Check cost limit
        if let Some(limit) = max_cost
            && cumulative_cost >= limit
        {
            emit_loop_stop(&format, "max_cost", &mut stdout)?;
            break;
        }

        let iter_num = iter_loop.display_iteration();
        let summary_before = iter_loop.spec_summary().suggest()?;

        if format == HeadlessFormat::Plain {
            writeln!(stderr, "--- iteration {iter_num} ---")?;
            writeln!(
                stderr,
                "spec: {}/{}",
                summary_before.checked, summary_before.total
            )?;
        }

        // Run one iteration
        let prompt = iter_loop.next_prompt(&wf.project_config)?;
        let config = super::cmd_next_headless::workflow_agent_config(&prompt, &dir, backend);

        let mut session = agent::session::AgentSession::start(config)
            .await
            .suggest()?;

        session.spawn_auto_approve_permissions();

        // Fire OnIterationStart lifecycle hooks
        crate::workflow::hook_dispatch::run_hooks_headless(
            &dir,
            &wf.project_config,
            &magistr_engine::LifecycleEvent::OnIterationStart,
        )
        .await;

        let iter_start = Instant::now();
        let (outcome, spend) = drain_iteration(
            &mut session,
            format,
            max_cost,
            &mut cumulative_cost,
            &mut stdout,
            &mut stderr,
        )
        .await?;
        final_outcome = outcome.clone();
        if let Some(spend) = spend {
            crate::cost::record_session(&dir, &spend);
        }
        let iter_duration = iter_start.elapsed();

        // Fire OnIterationEnd lifecycle hooks
        crate::workflow::hook_dispatch::run_hooks_headless(
            &dir,
            &wf.project_config,
            &magistr_engine::LifecycleEvent::OnIterationEnd,
        )
        .await;

        let summary_after = iter_loop
            .spec_summary()
            .unwrap_or_else(|_| summary_before.clone());

        // Emit per-iteration progress
        emit_iteration_progress(
            &format,
            iter_num,
            &outcome,
            &summary_before,
            &summary_after,
            iter_duration,
            &mut stdout,
        )?;

        // Post-iteration check
        match iter_loop.post_iteration_check() {
            PostCheckResult::CommitMade { .. } => {}
            PostCheckResult::NoCommit { .. } => {
                if iter_loop.should_stop_no_commits() {
                    emit_loop_stop(&format, "no_commits", &mut stdout)?;
                    break;
                }
            }
        }

        if matches!(
            outcome,
            HeadlessOutcome::AgentError | HeadlessOutcome::BudgetExceeded
        ) {
            iter_loop.mark_failed();
            emit_loop_stop(
                &format,
                match outcome {
                    HeadlessOutcome::AgentError => "agent_error",
                    HeadlessOutcome::BudgetExceeded => "budget_exceeded",
                    _ => unreachable!(),
                },
                &mut stdout,
            )?;
            break;
        }

        iter_loop.advance();
        iter_loop.persist().suggest()?;
        wf.cleanup()?;
    }

    // Summary report
    let loop_duration = loop_start.elapsed();
    emit_summary(
        &format,
        iter_loop.iteration(),
        max_iterations,
        loop_duration,
        &mut stdout,
    )?;

    iter_loop.cleanup().suggest()?;
    finish_loop(final_outcome)
}

/// Match the single-session exit contract after loop reporting and cleanup.
fn finish_loop(outcome: HeadlessOutcome) -> eyre::Result<()> {
    let code = outcome.exit_code();
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// Drain one session against the remaining loop budget and retain its reported spend.
async fn drain_iteration(
    session: &mut agent::session::AgentSession,
    format: HeadlessFormat,
    max_cost: Option<f64>,
    cumulative_cost: &mut f64,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> eyre::Result<(HeadlessOutcome, Option<crate::cost::SessionSpend>)> {
    let remaining = max_cost.map(|limit| (limit - *cumulative_cost).max(0.0));
    let mut spend = None;
    let outcome = drain_headless(session, format, remaining, out, err, &mut spend).await?;
    if let Some(spend) = &spend {
        *cumulative_cost += spend.cost_usd;
    }
    Ok((outcome, spend))
}

/// Emit a loop stop reason.
fn emit_loop_stop(format: &HeadlessFormat, reason: &str, w: &mut dyn Write) -> std::io::Result<()> {
    match format {
        HeadlessFormat::Plain => {
            writeln!(w, "\nloop stopped: {reason}")?;
        }
        HeadlessFormat::Json => {
            let msg = serde_json::json!({
                "type": "loop_stop",
                "reason": reason,
            });
            serde_json::to_writer(&mut *w, &msg).map_err(std::io::Error::other)?;
            writeln!(w)?;
        }
    }
    Ok(())
}

/// Emit per-iteration progress.
fn emit_iteration_progress(
    format: &HeadlessFormat,
    iteration: u32,
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
            writeln!(
                w,
                "iteration {iteration}: {status} (spec: {}/{} -> {}/{}, {:.1}s)",
                before.checked,
                before.total,
                after.checked,
                after.total,
                duration.as_secs_f64(),
            )?;
        }
        HeadlessFormat::Json => {
            let msg = serde_json::json!({
                "type": "iteration_progress",
                "iteration": iteration,
                "status": status,
                "spec_before": { "checked": before.checked, "total": before.total },
                "spec_after": { "checked": after.checked, "total": after.total },
                "duration_secs": duration.as_secs_f64(),
            });
            serde_json::to_writer(&mut *w, &msg).map_err(std::io::Error::other)?;
            writeln!(w)?;
        }
    }
    Ok(())
}

/// Emit the final loop summary.
fn emit_summary(
    format: &HeadlessFormat,
    iterations_completed: u32,
    max_iterations: u32,
    total_duration: std::time::Duration,
    w: &mut dyn Write,
) -> std::io::Result<()> {
    match format {
        HeadlessFormat::Plain => {
            writeln!(
                w,
                "loop complete: {iterations_completed}/{max_iterations} iterations, {:.1}s total",
                total_duration.as_secs_f64(),
            )?;
        }
        HeadlessFormat::Json => {
            let msg = serde_json::json!({
                "type": "loop_summary",
                "iterations_completed": iterations_completed,
                "max_iterations": max_iterations,
                "duration_secs": total_duration.as_secs_f64(),
            });
            serde_json::to_writer(&mut *w, &msg).map_err(std::io::Error::other)?;
            writeln!(w)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "cmd_next_loop_headless.tests.rs"]
mod tests;
