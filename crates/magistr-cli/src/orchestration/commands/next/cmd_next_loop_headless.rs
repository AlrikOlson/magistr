use std::io::Write;
use std::time::Instant;

use color_eyre::eyre;

use crate::agent;
use crate::cli::HeadlessFormat;
use crate::commands::run::{HeadlessOutcome, drain_headless_observing};
use crate::error::ResultExt;
use crate::workflow;
use crate::workflow::iteration::{
    IterationLoop, OperationKind, OperationOutcome, PostCheckResult, PreCheckResult,
};
use magistr_engine::workflow::StopReason;

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
    profile: crate::permissions::PermissionProfile,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = super::canonicalize_dir(dir)?;
    let inputs = super::workflow_inputs(&inputs.for_project(&dir), true)?;

    let wf = workflow::WorkflowConfig::load(&dir).suggest()?;
    wf.cleanup()?;

    let mut iter_loop = IterationLoop::start_or_recover(
        &dir,
        &wf.spec_path,
        max_iterations,
        crate::roadmap::admit_native_host(&dir)?,
    )
    .suggest()?;
    if let Some(selection) = inputs.session_selection() {
        iter_loop.bind_session_selection(selection.clone())?;
    }
    let max_iterations = iter_loop.max_iterations();
    iter_loop.set_budget_limit(max_cost).suggest()?;
    let max_cost = iter_loop.run_state().budget_limit_usd;
    iter_loop.persist().suggest()?;
    // The host's owner endpoint, for a cockpit opened beside this loop;
    // it lives for the loop and drops before the writer.
    let _owner_endpoint = crate::roadmap::serve_owner_endpoint(iter_loop.native()).await?;
    // The workspace this host owns for the loop: every iteration's session
    // writes through it, or is refused when it could not be admitted.
    let workspace =
        crate::commands::run::admit_workspace(&inputs, |line| eprintln!("{line}")).await;
    let source = workspace.source_route();

    let loop_start = Instant::now();
    let mut cumulative_cost = iter_loop.run_state().budget_total_usd;
    let mut final_outcome = HeadlessOutcome::Ok;
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();

    loop {
        let pre = iter_loop.pre_iteration_check().suggest()?;
        iter_loop.persist().suggest()?;
        match pre {
            PreCheckResult::Continue => {}
            PreCheckResult::Stopped(reason) => {
                final_outcome = match reason {
                    StopReason::BudgetExceeded => HeadlessOutcome::BudgetExceeded,
                    StopReason::Failed => HeadlessOutcome::AgentError,
                    StopReason::Cancelled | StopReason::Aborted => HeadlessOutcome::Cancelled,
                    _ => HeadlessOutcome::Ok,
                };
                emit_loop_stop(&format, reason.as_str(), &mut stdout)?;
                break;
            }
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
            PreCheckResult::Unreconciled(open) => {
                // Not a terminal outcome of the run: nothing is latched, and
                // the same command retries once the runtime is gone or the
                // checkpoint is explicitly set aside for a new run.
                emit_loop_stop(&format, "unreconciled", &mut stdout)?;
                let open: Vec<String> = open
                    .iter()
                    .map(|record| {
                        format!(
                            "{} {} attempt {}{}",
                            record.kind,
                            record.target,
                            record.attempt.number(),
                            record
                                .external
                                .as_deref()
                                .map(|handle| format!(" ({handle})"))
                                .unwrap_or_default()
                        )
                    })
                    .collect();
                return Err(eyre::eyre!(
                    "recovery could not establish what happened to: {} — reconcile before retrying, \
                     or preserve/remove .magistr/loop-state.json for an explicit new run",
                    open.join("; ")
                ));
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
        let config = super::cmd_next_headless::workflow_agent_config(&prompt, &dir, backend)
            .with_inputs(&inputs)
            .permission_profile(profile);

        // The dispatch, durable before the launch. An outcome already
        // established for this attempt (recovery saw the runtime's commit)
        // is the attempt's work, done: settle it, launch nothing.
        let dispatch = iter_loop
            .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
            .suggest()?;
        let Some(dispatch) = dispatch else {
            iter_loop.post_iteration_check();
            iter_loop.advance();
            iter_loop.persist().suggest()?;
            if format == HeadlessFormat::Plain {
                writeln!(
                    stderr,
                    "provider dispatch already established for this attempt — settled without launching again"
                )?;
            }
            continue;
        };
        iter_loop.persist().suggest()?;
        let mut session = match agent::session::AgentSession::start(config, source.clone())
            .await
            .suggest()
        {
            Ok(session) => session,
            Err(error) => {
                iter_loop
                    .settle_operation(&dispatch, OperationOutcome::Failed)
                    .suggest()?;
                iter_loop.post_iteration_check();
                iter_loop.advance();
                iter_loop.mark_failed();
                iter_loop.persist().suggest()?;
                release_workspace(workspace, source).await;
                return Err(error);
            }
        };
        // The runtime's handle, pinned to the particular process by its
        // start identity: what recovery asks about if this loop dies.
        iter_loop
            .start_operation(
                &dispatch,
                session
                    .provider_pid()
                    .and_then(workflow::runtime_handle::RuntimeHandle::observe)
                    .map(|handle| handle.to_string()),
            )
            .suggest()?;
        iter_loop.persist().suggest()?;

        // Fire OnIterationStart lifecycle hooks
        crate::workflow::hook_dispatch::run_hooks_headless(
            &dir,
            &wf.project_config,
            &magistr_engine::LifecycleEvent::OnIterationStart,
        )
        .await;

        let iter_start = Instant::now();
        let drained = drain_iteration(
            &mut session,
            format,
            max_cost,
            &mut cumulative_cost,
            &mut stdout,
            &mut stderr,
            &mut |total| {
                iter_loop.observe_budget(total);
                iter_loop.persist().suggest()?;
                Ok(())
            },
        )
        .await;
        // End provider residency before another attempt or a process exit.
        let cleanup = session.terminate_session().await.suggest();
        drop(session);
        if let Ok((_, Some(spend))) = &drained {
            crate::cost::record_session(&dir, spend);
        }
        let drained = drained.and_then(|result| cleanup.map(|()| result));
        let (outcome, _) = match drained {
            Ok(result) => result,
            Err(error) => {
                iter_loop
                    .settle_operation(&dispatch, OperationOutcome::Failed)
                    .suggest()?;
                iter_loop.post_iteration_check();
                iter_loop.advance();
                iter_loop.mark_failed();
                iter_loop.persist().suggest()?;
                release_workspace(workspace, source).await;
                return Err(error);
            }
        };
        final_outcome = outcome.clone();
        iter_loop
            .settle_operation(
                &dispatch,
                match outcome {
                    HeadlessOutcome::Ok => OperationOutcome::Completed,
                    HeadlessOutcome::Cancelled => OperationOutcome::Cancelled,
                    HeadlessOutcome::AgentError | HeadlessOutcome::BudgetExceeded => {
                        OperationOutcome::Failed
                    }
                },
            )
            .suggest()?;
        // Checkpoint the settled attempt before hooks and fallible progress output.
        let post = iter_loop.post_iteration_check();
        iter_loop.advance();
        let terminal = match outcome {
            HeadlessOutcome::AgentError => Some(StopReason::Failed),
            HeadlessOutcome::BudgetExceeded => Some(StopReason::BudgetExceeded),
            HeadlessOutcome::Cancelled => Some(StopReason::Cancelled),
            HeadlessOutcome::Ok => None,
        };
        if let Some(reason) = terminal {
            iter_loop.stop(reason);
        }
        iter_loop.persist().suggest()?;

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

        if matches!(
            outcome,
            HeadlessOutcome::AgentError
                | HeadlessOutcome::BudgetExceeded
                | HeadlessOutcome::Cancelled
        ) {
            emit_loop_stop(
                &format,
                match outcome {
                    HeadlessOutcome::AgentError => "agent_error",
                    HeadlessOutcome::BudgetExceeded => "budget_exceeded",
                    HeadlessOutcome::Cancelled => "cancelled",
                    _ => unreachable!(),
                },
                &mut stdout,
            )?;
            break;
        }

        if matches!(post, PostCheckResult::NoCommit { .. }) && iter_loop.should_stop_no_commits() {
            iter_loop.stop(StopReason::NoProgress);
            emit_loop_stop(&format, "no_commits", &mut stdout)?;
            break;
        }
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

    iter_loop.persist().suggest()?;
    // Released before the exit code below can end the process.
    release_workspace(workspace, source).await;
    finish_loop(final_outcome)
}

/// Release the loop's workspace admission and its source route, reporting
/// a refused shutdown on stderr rather than turning it into the loop's
/// outcome.
async fn release_workspace(
    workspace: crate::application::WorkspaceAdmission,
    source: magistr_application::effects::SourceRoute,
) {
    if let Err(err) = workspace.release(source).await {
        eprintln!("magistr: workspace shutdown: {err}");
    }
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
    checkpoint: &mut dyn FnMut(f64) -> eyre::Result<()>,
) -> eyre::Result<(HeadlessOutcome, Option<crate::cost::SessionSpend>)> {
    let base = *cumulative_cost;
    let remaining = max_cost.map(|limit| (limit - base).max(0.0));
    let mut spend = None;
    let outcome = drain_headless_observing(
        session,
        format,
        remaining,
        out,
        err,
        &mut spend,
        &mut |session_total| {
            *cumulative_cost = cumulative_cost.max(base + session_total);
            checkpoint(*cumulative_cost)
        },
    )
    .await?;
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
