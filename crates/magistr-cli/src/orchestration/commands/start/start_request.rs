use crate::agent::AgentConfig;

/// One `magistr start` invocation's session choices as the CLI parsed them:
/// the opening prompt and the resume/continue/model flags every run entry
/// point (plain, headless, cockpit) turns into the same [`AgentConfig`].
#[derive(Debug, Clone, Copy)]
pub struct StartRequest<'a> {
    pub prompt: &'a str,
    pub resume: Option<&'a str>,
    pub continue_conversation: bool,
    pub model: Option<&'a str>,
}

impl StartRequest<'_> {
    /// Apply the request's flags to a session configuration.
    pub fn apply(&self, config: AgentConfig) -> crate::error::Result<AgentConfig> {
        config.with_run_args(self.model, self.resume, self.continue_conversation)
    }
}
