use crate::agent;
use crate::cli::HeadlessFormat;
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
/// Aborts the session when cumulative cost reaches the captured budget
/// limit (the governed cap the host bound, or the `MAGISTR_BUDGET`
/// export a child process inherited).
pub async fn cmd_run_headless(
    request: super::RunRequest<'_>,
    backend: agent::Backend,
    format: HeadlessFormat,
    profile: crate::permissions::PermissionProfile,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let cwd = inputs.project_root().to_path_buf();

    let config = request
        .apply(agent::AgentConfig::new(request.prompt, &cwd).with_inputs(inputs))?
        .backend(backend)
        .permission_profile(profile)
        .with_project_context(&cwd);

    // Admitted, the session's source writes run through the pipeline;
    // refused, they are refused (stderr carries the reason as metadata).
    let workspace = super::admit_workspace(inputs, |line| eprintln!("{line}")).await;
    let mut session = agent::session::AgentSession::start(config, workspace.source_route())
        .await
        .suggest()?;

    let budget_limit = inputs.budget_limit();

    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();

    let drained =
        drain_headless(&mut session, format, budget_limit, &mut stdout, &mut stderr).await;
    let cleanup = session.terminate_session().await.suggest();
    drop(session);
    if let Err(err) = workspace.shutdown().await {
        eprintln!("magistr: workspace shutdown: {err}");
    }
    let outcome = drained?;

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
/// The shared session owner saves spending independently of this output reader.
pub(crate) async fn drain_headless(
    session: &mut agent::session::AgentSession,
    format: HeadlessFormat,
    budget_limit: Option<f64>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> eyre::Result<HeadlessOutcome> {
    drain_headless_observing(session, format, budget_limit, out, err, &mut |_| Ok(())).await
}

/// Observe reported spending before output or termination for loop checkpoints.
pub(crate) async fn drain_headless_observing(
    session: &mut agent::session::AgentSession,
    format: HeadlessFormat,
    budget_limit: Option<f64>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    observe_cost: &mut dyn FnMut(f64) -> eyre::Result<()>,
) -> eyre::Result<HeadlessOutcome> {
    use crate::agent::message::AgentMessage;
    let mut renderer = super::plain_renderer::PlainRenderer::default();
    while let Some(message) = session.next_message().await {
        let msg = match message {
            Ok(msg) => msg,
            Err(error) => {
                renderer.finish(out, err)?;
                return Err(eyre::eyre!("Agent stream error: {error}"));
            }
        };
        let cost = match &msg {
            AgentMessage::Result { total_cost_usd, .. } => *total_cost_usd,
            AgentMessage::UsageUpdate { cost_usd, .. } => *cost_usd,
            _ => None,
        };
        if let Some(cost) = cost.filter(|cost| cost.is_finite() && *cost >= 0.0) {
            observe_cost(cost)?;
        }
        match format {
            HeadlessFormat::Plain => renderer.render(&msg, out, err)?,
            HeadlessFormat::Json => render_json(&msg, out)?,
        }
        match &msg {
            AgentMessage::Result {
                is_error,
                total_cost_usd,
                ..
            } => {
                if budget_limit
                    .zip(*total_cost_usd)
                    .is_some_and(|(limit, cost)| cost >= limit)
                {
                    return Ok(HeadlessOutcome::BudgetExceeded);
                }
                return Ok(if *is_error {
                    HeadlessOutcome::AgentError
                } else {
                    HeadlessOutcome::Ok
                });
            }
            AgentMessage::UsageUpdate {
                cost_usd: Some(cost),
                ..
            } if budget_limit.is_some_and(|limit| *cost >= limit) => {
                let limit = budget_limit.expect("matched reported cost against limit");
                let message = format!("budget limit exceeded (${cost:.4} >= ${limit:.4})");
                match format {
                    HeadlessFormat::Plain => {
                        renderer.finish(out, err)?;
                        writeln!(err, "error: {message}")?;
                        err.flush()?;
                    }
                    HeadlessFormat::Json => {
                        write_ndjson(out, &serde_json::json!({"type":"error","message":message}))?
                    }
                }
                session.cancel_turn()?;
                return Ok(HeadlessOutcome::BudgetExceeded);
            }
            AgentMessage::Cancelled { .. } => return Ok(HeadlessOutcome::Cancelled),
            _ => {}
        }
    }
    renderer.finish(out, err)?;
    Ok(HeadlessOutcome::AgentError)
}

fn render_json(
    msg: &crate::agent::message::AgentMessage,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    use crate::agent::message::{AgentMessage, ContentBlock, UserBody};
    let blocks = match msg {
        AgentMessage::Assistant { message, .. } => Some(&message.content),
        AgentMessage::User { message, .. } => match &message.content {
            Some(UserBody::Blocks(blocks)) => Some(blocks),
            _ => None,
        },
        _ => None,
    };
    if let Some(blocks) = blocks {
        for block in blocks {
            let value = match block {
                ContentBlock::Text { text } if matches!(msg, AgentMessage::Assistant { .. }) => {
                    serde_json::json!({"type":"text","content":text})
                }
                ContentBlock::ToolUse {
                    id, name, input, ..
                } => serde_json::json!({"type":"tool_use","id":id,"name":name,"input":input}),
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    serde_json::json!({"type":"tool_result","id":tool_use_id,"content":content,"is_error":is_error})
                }
                _ => continue,
            };
            write_ndjson(out, &value)?;
        }
    }
    let value = match msg {
        AgentMessage::Result {
            is_error,
            num_turns,
            duration_ms,
            total_cost_usd,
            session_id,
            result,
            ..
        } => {
            serde_json::json!({"type":"result","is_error":is_error,"num_turns":num_turns,"duration_secs":*duration_ms as f64 / 1000.0,"total_cost_usd":total_cost_usd,"session_id":session_id.as_str(),"result":result})
        }
        AgentMessage::UsageUpdate {
            cost_usd,
            context_tokens_used,
            context_window_size,
        } => {
            serde_json::json!({"type":"usage","cost_usd":cost_usd,"context_tokens_used":context_tokens_used,"context_window_size":context_window_size})
        }
        AgentMessage::Cancelled { reason, .. } => {
            serde_json::json!({"type":"cancelled","reason":reason})
        }
        AgentMessage::System { subtype, data } if subtype == "terminal_output" => {
            serde_json::json!({"type":"terminal_output","data":data})
        }
        AgentMessage::StreamEvent { event, .. } => {
            serde_json::json!({"type":"tool_update","event":event})
        }
        AgentMessage::Plan { entries } => {
            serde_json::json!({"type":"plan","entries":entries.iter().map(|entry| serde_json::json!({"content":entry.content,"status":format!("{:?}",entry.status)})).collect::<Vec<_>>()})
        }
        _ => return Ok(()),
    };
    write_ndjson(out, &value)
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
