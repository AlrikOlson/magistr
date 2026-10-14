//! New planning conversation setup for requests made from Roadmap.

use magistr_application::sessions::SessionPurpose;

use crate::agent::{AgentConfig, persistence::SessionHistory};

/// Create a new planning conversation without replacing user provider/model choices.
/// Shared session resolution applies its purpose's tools, instructions and policy.
pub(crate) fn config(base: &AgentConfig) -> AgentConfig {
    let mut config = base.clone();
    config.prompt.clear();
    config.history = SessionHistory::for_purpose(SessionPurpose::Roadmap);
    config.resume = None;
    config.continue_conversation = false;
    config
}

#[cfg(test)]
#[path = "roadmap_request.tests.rs"]
mod tests;
