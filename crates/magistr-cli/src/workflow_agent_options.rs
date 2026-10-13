//! Shared command options and preflight for native agent sessions.

use crate::agent::AgentConfig;
use crate::config::inputs::ProjectInputs;
use crate::config::session_selection::SessionSelection;
use crate::workflow::iteration::IterationState;
use clap::Args;
use color_eyre::eyre;

#[derive(Debug, Clone, Default, Args)]
pub struct WorkflowAgentOptions {
    /// Agent to use: auto, claude, copilot, or codex
    #[arg(long, alias = "provider")]
    pub agent: Option<String>,
    /// Model offered by the selected agent; use default to let the agent choose
    #[arg(long, short = 'm')]
    pub model: Option<String>,
    /// Reasoning effort offered by the selected agent; use default to let it choose
    #[arg(long)]
    pub effort: Option<String>,
    /// Show the selected agent settings and exit without starting work
    #[arg(long)]
    pub show_config: bool,
}

impl WorkflowAgentOptions {
    pub fn prepare(
        &self,
        inputs: &ProjectInputs,
        looping: bool,
        resume: Option<&str>,
        continuing: bool,
    ) -> eyre::Result<ProjectInputs> {
        let mut inputs = inputs.clone();
        if looping
            && let Some(state) = IterationState::read(inputs.project_root())?
            && (state.run.terminal_reason.is_none()
                || state.run.terminal_reason == Some(magistr_engine::workflow::StopReason::Paused))
            && let Some(saved) = state.session_selection
        {
            self.check_recovery(&saved)?;
            inputs = inputs.with_session_selection(saved);
        }
        let mut config = AgentConfig::new("", inputs.project_root()).with_inputs(&inputs);
        if let Some(agent) = &self.agent {
            config = config.provider(agent);
        }
        if let Some(effort) = &self.effort {
            config = config.effort(effort);
        }
        let config = config.with_run_args(self.model.as_deref(), resume, continuing)?;
        let spec = config.resolve()?;
        let mut selection = spec.selection();
        if let Some(saved) = inputs.session_selection() {
            selection.sources.clone_from(&saved.sources);
        }
        Ok(inputs.with_session_selection(selection))
    }

    fn check_recovery(&self, saved: &SessionSelection) -> eyre::Result<()> {
        let mismatch = self
            .agent
            .as_deref()
            .is_some_and(|agent| agent != saved.agent)
            || self
                .model
                .as_deref()
                .is_some_and(|model| normalize(model) != saved.model.as_deref())
            || self
                .effort
                .as_deref()
                .is_some_and(|effort| normalize(effort) != saved.effort.as_deref());
        if mismatch {
            eyre::bail!(
                "This unfinished run uses saved agent settings. Resume without changing --agent, --model, or --effort; change settings for a new run after it has ended"
            );
        }
        Ok(())
    }

    pub fn show(inputs: &ProjectInputs) -> eyre::Result<()> {
        let selection = inputs
            .session_selection()
            .ok_or_else(|| eyre::eyre!("Agent settings have not been selected"))?;
        println!("{}", serde_json::to_string_pretty(selection)?);
        Ok(())
    }

    pub fn announce(inputs: &ProjectInputs) {
        if let Some(selection) = inputs.session_selection() {
            eprintln!(
                "Agent: {} ({}) · model: {} ({}) · effort: {} ({})",
                selection.agent,
                selection.source("provider"),
                selection.model.as_deref().unwrap_or("agent default"),
                selection.source("model"),
                selection.effort.as_deref().unwrap_or("agent default"),
                selection.source("effort")
            );
        }
    }
}

fn normalize(value: &str) -> Option<&str> {
    (!value.eq_ignore_ascii_case("default")).then_some(value)
}

#[cfg(test)]
#[path = "workflow_agent_options.tests.rs"]
mod tests;
