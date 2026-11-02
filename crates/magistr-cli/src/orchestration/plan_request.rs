//! New planning conversation setup for requests made from Plan.

use magistr_application::sessions::SessionPurpose;

use crate::agent::{AgentConfig, persistence::SessionHistory};

/// Create a new planning conversation without replacing user provider/model choices.
/// Shared session resolution applies its purpose's tools, instructions and policy.
pub(crate) fn config(base: &AgentConfig) -> AgentConfig {
    let mut config = base.clone();
    config.prompt.clear();
    config.history = SessionHistory::for_purpose(SessionPurpose::Plan);
    config.resume = None;
    config
}

#[cfg(test)]
#[path = "plan_request.tests.rs"]
mod tests;
