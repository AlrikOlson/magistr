//! Freeze or recover agent settings before a native workflow can start.

use crate::config::inputs::ProjectInputs;
use crate::workflow_agent_options::WorkflowAgentOptions;
use color_eyre::eyre;

pub(super) fn workflow_inputs(
    inputs: &ProjectInputs,
    looping: bool,
) -> eyre::Result<ProjectInputs> {
    let prepared = if inputs.session_selection().is_some() {
        inputs.clone()
    } else {
        WorkflowAgentOptions::default().prepare(inputs, looping, None, false)?
    };
    let agent = prepared
        .session_selection()
        .expect("prepare freezes a selection")
        .agent
        .as_str();
    if !["claude", "copilot", "codex"].contains(&agent) {
        eyre::bail!(
            "Agent {agent} cannot edit this project. Choose claude, copilot, or codex for next and next-loop"
        );
    }
    Ok(prepared)
}
