//! Run/agent commands: run, run TUI, and the ACP permission handler.

use std::path::Path;

use crate::agent;
use crate::cost::{SessionSpend, record_session};
use crate::utils::format as fmt;
use color_eyre::eyre;

/// Interactive session loop — drains turns and prompts for follow-ups via stdin.
///
/// This is the full interactive loop used by `cmd_run` in no-TUI mode.
///
/// Each completed turn is written to the cost ledger (found through the
/// captured `inputs`), stamped with the resolved identity and `model` —
/// this and the headless drain are the only two places the agent's
/// result message is seen, so they are the only two places the ledger
/// can be fed.
async fn drain_session_interactive(
    session: &mut agent::session::AgentSession,
    dir: &Path,
    inputs: &crate::config::inputs::ProjectInputs,
    model: Option<&str>,
) -> eyre::Result<()> {
    loop {
        // Drain messages until the turn completes (Result) or stream ends
        let mut turn_done = false;
        while let Some(msg_result) = session.next_message().await {
            match msg_result {
                Ok(msg) => match &msg {
                    crate::agent::message::AgentMessage::Assistant { message, .. } => {
                        for block in &message.content {
                            if let crate::agent::message::ContentBlock::Text { text } = block {
                                print!("{text}");
                            }
                        }
                    }
                    crate::agent::message::AgentMessage::Result {
                        is_error,
                        num_turns,
                        duration_ms,
                        total_cost_usd,
                        session_id,
                        usage,
                        ..
                    } => {
                        let mut spend =
                            SessionSpend::new(session_id.as_str(), total_cost_usd.unwrap_or(0.0))
                                .with_run(*num_turns, *duration_ms)
                                .with_tokens(
                                    usage
                                        .as_ref()
                                        .map(SessionSpend::tokens_from_usage)
                                        .unwrap_or_default(),
                                );
                        if let Some(model) = model {
                            spend = spend.with_model(model);
                        }
                        record_session(inputs, dir, &spend);

                        let duration_secs = *duration_ms as f64 / 1000.0;
                        let cost_str = total_cost_usd
                            .map(|c| format!("${c:.4}"))
                            .unwrap_or_else(|| "unknown".into());
                        eprintln!();
                        if *is_error {
                            eprintln!("{}", fmt::error("Agent ended with error"));
                        } else {
                            eprintln!(
                                "{}",
                                fmt::success(&format!(
                                    "Done — {num_turns} turns, {duration_secs:.1}s, cost: {cost_str}"
                                ))
                            );
                            eprintln!(
                                "  {}",
                                fmt::dim(&format!("Session: {}", session_id.as_str()))
                            );
                        }
                        turn_done = true;
                        break;
                    }
                    crate::agent::message::AgentMessage::Cancelled { reason, .. } => {
                        return Err(eyre::eyre!("Agent turn cancelled: {reason}"));
                    }
                    _ => {}
                },
                Err(e) => {
                    return Err(eyre::eyre!("Agent stream error: {e}"));
                }
            }
        }

        if !turn_done {
            return Err(eyre::eyre!(
                "Agent stream ended without a terminal response"
            ));
        }

        // Prompt for follow-up input
        eprint!("{} Follow up (empty to quit): ", fmt::cyan("→"));
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
            break;
        }

        session.send_followup(input.trim())?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;

mod admit_workspace;
mod cmd_run;
#[cfg(feature = "terminal")]
mod cmd_run_cockpit;
mod cmd_run_headless;
#[cfg(feature = "terminal")]
mod cmd_run_tui;
mod cmd_run_workflow;
mod cockpit_policy_for_project;
mod collect_gate_commands;
mod drain_result;
mod drain_session_turn;
mod protected_paths_for_project;
mod resolve_permission_profile;
mod run_request;
mod seed_supervision;

pub(crate) use admit_workspace::admit_workspace;
pub use cmd_run::cmd_run;
#[cfg(feature = "terminal")]
pub use cmd_run_cockpit::cmd_run_cockpit;
pub use cmd_run_headless::cmd_run_headless;
pub(crate) use cmd_run_headless::{HeadlessOutcome, drain_headless, drain_headless_observing};
#[cfg(feature = "terminal")]
pub use cmd_run_tui::cmd_run_tui;
pub use cmd_run_workflow::cmd_run_workflow;
pub use cockpit_policy_for_project::cockpit_policy_for_project;
pub use collect_gate_commands::collect_gate_commands;
pub use drain_result::DrainResult;
pub use drain_session_turn::drain_session_turn;
pub use protected_paths_for_project::protected_paths_for_project;
pub use resolve_permission_profile::resolve_permission_profile;
pub use run_request::RunRequest;
pub use seed_supervision::seed_supervision;
