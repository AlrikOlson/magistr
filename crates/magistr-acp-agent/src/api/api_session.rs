//! What an API provider keeps per session.

use crate::{ChatMessage, Role};

/// The history every request carries and the running totals the usage
/// update reports. Lives only as long as the agent process: none of these
/// APIs holds a session, so `session/load` starts from nothing.
#[derive(Debug, Default)]
pub(crate) struct ApiSession {
    /// The conversation so far, user and assistant alternating.
    pub(crate) history: Vec<ChatMessage>,
    /// Prompt tokens across every turn.
    pub(crate) input_tokens: u64,
    /// Completion tokens across every turn.
    pub(crate) output_tokens: u64,
    /// The session's cumulative cost, in USD.
    pub(crate) cost_usd: f64,
}

impl ApiSession {
    /// Open a turn: the prompt joins the history, and the whole history is
    /// what the request carries.
    pub(crate) fn begin_turn(&mut self, prompt: impl Into<String>) -> Vec<ChatMessage> {
        self.history.push(ChatMessage::new(Role::User, prompt));
        self.history.clone()
    }

    /// Close a turn with the reply and its usage; the cost is added at the
    /// model's pricing by the caller and passed in.
    pub(crate) fn end_turn(&mut self, reply: String, usage: Option<(u64, u64)>, cost: f64) {
        self.history.push(ChatMessage::new(Role::Assistant, reply));
        if let Some((input, output)) = usage {
            self.input_tokens += input;
            self.output_tokens += output;
        }
        self.cost_usd += cost;
    }

    /// Close a cancelled or failed turn. A partial reply stays in the
    /// history as what the user saw; an empty one takes its prompt with it,
    /// so the history keeps alternating (Gemini refuses one that does not).
    pub(crate) fn abandon_turn(&mut self, partial: String) {
        if partial.is_empty() {
            self.history.pop();
        } else {
            self.history
                .push(ChatMessage::new(Role::Assistant, partial));
        }
    }
}

#[cfg(test)]
#[path = "api_session.tests.rs"]
mod tests;
