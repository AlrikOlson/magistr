//! Start commands read shared session observations and route permission handling.

use crate::agent;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// Interactive session loop — drains turns and prompts for follow-ups via stdin.
///
/// This is the full interactive loop used by `cmd_start` in no-TUI mode.
///
/// Spending observations are resolved and saved by the shared session owner.
async fn drain_session_interactive(session: &mut agent::session::AgentSession) -> eyre::Result<()> {
    loop {
        drain_session_plain_turn(session).await?;
        // Follow-up input belongs only to the interactive run command.
        eprint!("{} Follow up (empty to quit): ", fmt::cyan("→"));
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
            break;
        }
        session.send_followup(&input.trim().into())?;
    }
    Ok(())
}

/// Workflow commands finish after one turn; the orchestrator owns continuation.
async fn drain_session_plain_turn(session: &mut agent::session::AgentSession) -> eyre::Result<()> {
    let mut out = std::io::stdout().lock();
    let mut err = std::io::stderr().lock();
    match drain_headless(
        session,
        crate::cli::HeadlessFormat::Plain,
        None,
        &mut out,
        &mut err,
    )
    .await?
    {
        HeadlessOutcome::Ok => Ok(()),
        HeadlessOutcome::Cancelled => Err(eyre::eyre!("Agent turn cancelled")),
        HeadlessOutcome::AgentError => Err(eyre::eyre!(
            "Agent ended with error or stream ended without a terminal response"
        )),
        HeadlessOutcome::BudgetExceeded => Err(eyre::eyre!("Spending limit reached")),
    }
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;

mod admit_workspace;
mod cmd_start;
mod cmd_start_headless;
mod cmd_start_workflow;
mod cockpit_policy_for_project;
mod collect_gate_commands;
mod drain_result;
mod drain_session_turn;
mod plain_output;
mod plain_renderer;
mod protected_paths_for_project;
mod resolve_permission_profile;
mod seed_supervision;
mod start_request;

pub(crate) use admit_workspace::admit_workspace;
pub use cmd_start::cmd_start;
pub use cmd_start_headless::cmd_start_headless;
pub(crate) use cmd_start_headless::{HeadlessOutcome, drain_headless, drain_headless_observing};
pub use cmd_start_workflow::cmd_start_workflow;
pub use cockpit_policy_for_project::cockpit_policy_for_project;
pub use collect_gate_commands::collect_gate_commands;
pub use drain_result::DrainResult;
pub use drain_session_turn::drain_session_turn;
pub use protected_paths_for_project::protected_paths_for_project;
pub use resolve_permission_profile::resolve_permission_profile;
pub use seed_supervision::supervision_seed;
pub use start_request::StartRequest;
