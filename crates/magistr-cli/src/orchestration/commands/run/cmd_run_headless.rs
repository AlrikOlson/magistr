use crate::agent;
use crate::cli::HeadlessFormat;
use crate::cost::{SessionSpend, record_session};
use crate::error::ResultExt;
use color_eyre::eyre;
use std::io::Write;

/// Outcome of a headless session drain.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HeadlessOutcome {
    /// Session completed normally.
    Ok,
    /// Agent ended with an error.
    AgentError,
    /// Session was cancelled (user or system initiated).
    Cancelled,
    /// Budget limit was exceeded.
    BudgetExceeded,
}

impl HeadlessOutcome {
    /// Map outcome to a process exit code.
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::Ok | Self::Cancelled => 0,
            Self::AgentError => 1,
            Self::BudgetExceeded => 2,
        }
    }
}

/// Run an agent session in headless mode — no TUI, no interactive prompts.
///
/// Streams agent output to stdout (text in plain mode, NDJSON in JSON mode).
/// Metadata (tool calls, summary) goes to stderr in plain mode.
/// Applies the shared project policy; unanswered approvals are refused by default.
///
/// Respects `MAGISTR_BUDGET` env var for cost limits — aborts the session
/// if cumulative cost exceeds the budget.
pub async fn cmd_run_headless(
    prompt: &str,
    resume: Option<&str>,
    continue_conversation: bool,
    model: Option<&str>,
    backend: agent::Backend,
    format: HeadlessFormat,
    profile: crate::permissions::PermissionProfile,
) -> eyre::Result<()> {
    let cwd = std::env::current_dir()?;

    let config = agent::AgentConfig::new(prompt, &cwd)
        .backend(backend)
        .permission_profile(profile)
        .with_run_args(model, resume, continue_conversation)
        .with_project_context(&cwd);

    // Admitted, the session's source writes run through the pipeline;
    // refused, they are refused (stderr carries the reason as metadata).
    let workspace = super::admit_workspace(&cwd, |line| eprintln!("{line}")).await;
    let mut session = agent::session::AgentSession::start(config, workspace.source_route())
        .await
        .suggest()?;
    let model = session.resolved_model().map(str::to_owned);

    let budget_limit: Option<f64> = std::env::var("MAGISTR_BUDGET")
        .ok()
        .and_then(|v| v.parse().ok());

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

    // The ledger every budget scope is measured over. Written here
    // rather than inside the drain so the drain stays a pure function of
    // its writers and its tests never touch the real ledger.
    if let Some(spend) = spend.map(|s| model.as_deref().map_or(s.clone(), |m| s.with_model(m))) {
        record_session(&cwd, &spend);
    }
    cleanup?;

    // Settled — the ledger row written, the workspace released — an
    // interrupted run exits as the interrupt it was.
    crate::utils::shutdown::Shutdown::exit_if_requested();
    let code = outcome.exit_code();
    if code != 0 {
        std::process::exit(code);
    }

    Ok(())
}

/// Drain all messages from the session in headless mode.
///
/// Writes output to the provided writers (testable). Returns the session outcome
/// instead of calling `process::exit` directly.
///
/// `spend` receives what the session's result message reported, for the
/// caller to write to the cost ledger. It is an out-parameter rather
/// than a return value because the drain already has a meaningful
/// return, and because a drain that ends without a result message must
/// leave it untouched rather than invent a zero row.
pub(crate) async fn drain_headless(
    session: &mut agent::session::AgentSession,
    format: HeadlessFormat,
    budget_limit: Option<f64>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    spend: &mut Option<SessionSpend>,
) -> eyre::Result<HeadlessOutcome> {
    drain_headless_observing(session, format, budget_limit, out, err, spend, &mut |_| {
        Ok(())
    })
    .await
}

/// Observe priced usage before output or termination so a loop can checkpoint
/// reported spending even if no final ledger result arrives.
pub(crate) async fn drain_headless_observing(
    session: &mut agent::session::AgentSession,
    format: HeadlessFormat,
    budget_limit: Option<f64>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    spend: &mut Option<SessionSpend>,
    observe_cost: &mut dyn FnMut(f64) -> eyre::Result<()>,
) -> eyre::Result<HeadlessOutcome> {
    use crate::agent::message::{AgentMessage, ContentBlock};

    while let Some(msg_result) = session.next_message().await {
        if let Ok(msg) = &msg_result {
            let cost = match msg {
                AgentMessage::Result { total_cost_usd, .. } => *total_cost_usd,
                AgentMessage::UsageUpdate { cost_usd, .. } => *cost_usd,
                _ => None,
            };
            if let Some(cost) = cost.filter(|cost| cost.is_finite() && *cost >= 0.0) {
                observe_cost(cost)?;
            }
        }
        match msg_result {
            Ok(msg) => match &msg {
                AgentMessage::Assistant { message, .. } => {
                    for block in &message.content {
                        match block {
                            ContentBlock::Text { text } => match format {
                                HeadlessFormat::Plain => {
                                    write!(out, "{text}")?;
                                    out.flush()?;
                                }
                                HeadlessFormat::Json => {
                                    write_ndjson(
                                        out,
                                        &serde_json::json!({
                                            "type": "text",
                                            "content": text,
                                        }),
                                    )?;
                                }
                            },
                            ContentBlock::ToolUse { name, input, .. } => match format {
                                HeadlessFormat::Plain => {
                                    writeln!(err, "[tool] {name}")?;
                                }
                                HeadlessFormat::Json => {
                                    write_ndjson(
                                        out,
                                        &serde_json::json!({
                                            "type": "tool_use",
                                            "name": name,
                                            "input": input,
                                        }),
                                    )?;
                                }
                            },
                            ContentBlock::ToolResult {
                                content, is_error, ..
                            } if format == HeadlessFormat::Json => {
                                write_ndjson(
                                    out,
                                    &serde_json::json!({
                                        "type": "tool_result",
                                        "content": content,
                                        "is_error": is_error,
                                    }),
                                )?;
                            }
                            _ => {}
                        }
                    }
                }
                AgentMessage::Result {
                    is_error,
                    num_turns,
                    duration_ms,
                    total_cost_usd,
                    session_id,
                    usage,
                    ..
                } => {
                    let duration_secs = *duration_ms as f64 / 1000.0;
                    *spend = Some(
                        SessionSpend::new(session_id.as_str(), total_cost_usd.unwrap_or(0.0))
                            .with_run(*num_turns, *duration_ms)
                            .with_tokens(
                                usage
                                    .as_ref()
                                    .map(SessionSpend::tokens_from_usage)
                                    .unwrap_or_default(),
                            ),
                    );
                    match format {
                        HeadlessFormat::Plain => {
                            writeln!(out)?;
                            let cost_str = total_cost_usd
                                .map(|c| format!("${c:.4}"))
                                .unwrap_or_else(|| "unknown".into());
                            if *is_error {
                                writeln!(
                                    err,
                                    "error: agent ended with error ({num_turns} turns, {duration_secs:.1}s, cost: {cost_str})"
                                )?;
                            } else {
                                writeln!(
                                    err,
                                    "done: {num_turns} turns, {duration_secs:.1}s, cost: {cost_str}, session: {}",
                                    session_id.as_str()
                                )?;
                            }
                        }
                        HeadlessFormat::Json => {
                            write_ndjson(
                                out,
                                &serde_json::json!({
                                    "type": "result",
                                    "is_error": is_error,
                                    "num_turns": num_turns,
                                    "duration_secs": duration_secs,
                                    "total_cost_usd": total_cost_usd,
                                    "session_id": session_id.as_str(),
                                }),
                            )?;
                        }
                    }
                    if budget_limit
                        .zip(*total_cost_usd)
                        .is_some_and(|(limit, cost)| cost >= limit)
                    {
                        return Ok(HeadlessOutcome::BudgetExceeded);
                    }
                    if *is_error {
                        return Ok(HeadlessOutcome::AgentError);
                    }
                    return Ok(HeadlessOutcome::Ok);
                }
                AgentMessage::UsageUpdate {
                    cost_usd,
                    context_tokens_used,
                    context_window_size,
                    ..
                } => {
                    if format == HeadlessFormat::Json {
                        write_ndjson(
                            out,
                            &serde_json::json!({
                                "type": "usage",
                                "cost_usd": cost_usd,
                                "context_tokens_used": context_tokens_used,
                                "context_window_size": context_window_size,
                            }),
                        )?;
                    }

                    // Budget enforcement
                    if let (Some(limit), Some(cost)) = (budget_limit, cost_usd)
                        && *cost >= limit
                    {
                        match format {
                            HeadlessFormat::Plain => {
                                writeln!(
                                    err,
                                    "error: budget limit exceeded (${cost:.4} >= ${limit:.4})"
                                )?;
                            }
                            HeadlessFormat::Json => {
                                write_ndjson(
                                    out,
                                    &serde_json::json!({
                                        "type": "error",
                                        "message": format!("budget limit exceeded (${cost:.4} >= ${limit:.4})"),
                                    }),
                                )?;
                            }
                        }
                        session.cancel_turn()?;
                        return Ok(HeadlessOutcome::BudgetExceeded);
                    }
                }
                AgentMessage::Cancelled { reason, .. } => {
                    match format {
                        HeadlessFormat::Plain => {
                            writeln!(err, "cancelled: {reason}")?;
                        }
                        HeadlessFormat::Json => {
                            write_ndjson(
                                out,
                                &serde_json::json!({
                                    "type": "cancelled",
                                    "reason": reason,
                                }),
                            )?;
                        }
                    }
                    return Ok(HeadlessOutcome::Cancelled);
                }
                _ => {}
            },
            Err(e) => {
                return Err(eyre::eyre!("Agent stream error: {e}"));
            }
        }
    }

    // EOF without a terminal result is an interrupted attempt, not success.
    Ok(HeadlessOutcome::AgentError)
}

/// Write a JSON value as a single NDJSON line.
fn write_ndjson(w: &mut dyn Write, value: &serde_json::Value) -> std::io::Result<()> {
    let line = serde_json::to_string(value).map_err(std::io::Error::other)?;
    writeln!(w, "{line}")?;
    w.flush()
}

#[cfg(test)]
#[path = "cmd_run_headless.tests.rs"]
mod tests;
