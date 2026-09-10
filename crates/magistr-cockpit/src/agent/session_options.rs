//! Startup choices consumed by the host's session owner.

/// The session owner's opening moves: the model to select before anything is
/// sent, and the first prompt to send once the agent is ready. The cockpit relay
/// receives the resulting events and never performs these operations itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionOptions {
    /// The prompt to send as soon as the session is initialized — what
    /// `magistr run <prompt>` was given. `None` waits for the user.
    pub opening_prompt: Option<String>,
    /// The model to select through `session/set_config_option` before
    /// the opening prompt, when the agent advertises a model option.
    pub model: Option<String>,
}

impl SessionOptions {
    /// Wait for the user; the agent's default model.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Send `prompt` once the session is initialized.
    #[must_use]
    pub fn opening_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.opening_prompt = Some(prompt.into());
        self
    }

    /// Select `model` before the opening prompt.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}
