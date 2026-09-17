//! Agent activity derived only from received events and submitted requests.

use std::collections::BTreeMap;

use magistr_application::sessions::{AgentEvent, ToolStatus, ToolVerb, TurnEnd};

/// A human-readable task and its current, observed activity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentActivity {
    /// The kind of work being performed.
    pub task: String,
    /// What the agent or its command owner last reported.
    pub status: &'static str,
    /// Whether an answer or action is still in progress.
    pub active: bool,
    tools: BTreeMap<String, (ToolVerb, ToolStatus)>,
}

#[cfg(test)]
#[path = "agent_activity.tests.rs"]
mod tests;

impl Default for AgentActivity {
    fn default() -> Self {
        Self {
            task: "Conversation".to_owned(),
            status: "Ready",
            active: false,
            tools: BTreeMap::new(),
        }
    }
}

impl AgentActivity {
    pub(super) fn begin(&mut self, task: &str) {
        self.task = task.to_owned();
        self.status = "Waiting for response";
        self.active = true;
        self.tools.clear();
    }

    pub(super) fn stopping(&mut self) {
        if self.active {
            self.status = "Stopping";
        }
    }

    pub(super) fn observe(&mut self, event: &AgentEvent) {
        let stopping = self.active && self.status == "Stopping";
        match event {
            AgentEvent::Text(_) | AgentEvent::Image(_) => {
                self.active = true;
                self.status = "Writing a response";
            }
            AgentEvent::Thought(_) => {
                self.active = true;
                self.status = "Thinking";
            }
            AgentEvent::UserText(_) => {
                self.active = true;
                self.status = "Waiting for response";
            }
            AgentEvent::ToolCall {
                id, verb, status, ..
            } => {
                self.active = true;
                if matches!(status, ToolStatus::Pending | ToolStatus::InProgress) {
                    self.tools.insert(id.clone(), (*verb, *status));
                } else {
                    self.tools.remove(id);
                }
                self.status = "Waiting for response";
            }
            AgentEvent::ToolCallUpdate {
                id,
                status: Some(status),
                ..
            } => {
                if matches!(status, ToolStatus::Completed | ToolStatus::Failed) {
                    self.tools.remove(id);
                    self.status = "Waiting for response";
                } else if let Some((_, prior)) = self.tools.get_mut(id) {
                    *prior = *status;
                }
            }
            AgentEvent::TurnEnded(end) => {
                self.active = false;
                self.tools.clear();
                self.status = match end {
                    TurnEnd::EndTurn => "Response complete",
                    TurnEnd::Cancelled | TurnEnd::Other => "Stopped",
                    TurnEnd::Refusal => "Request declined",
                    TurnEnd::MaxTokens | TurnEnd::MaxTurnRequests => "Limit reached",
                };
            }
            AgentEvent::Error(_) | AgentEvent::PromptRejected { .. } => {
                self.active = false;
                self.tools.clear();
                self.status = "Could not finish";
            }
            _ => {}
        }
        if self.active && stopping {
            self.status = "Stopping";
        }
        if self.active
            && self.status != "Stopping"
            && let Some((verb, status)) = self
                .tools
                .values()
                .find(|(_, status)| *status == ToolStatus::InProgress)
                .or_else(|| self.tools.values().next())
        {
            self.status = match (status, verb) {
                (ToolStatus::Pending, _) => "Waiting for an action",
                (_, ToolVerb::Read) => "Reading",
                (_, ToolVerb::Search) => "Searching",
                (_, ToolVerb::Edit | ToolVerb::Delete | ToolVerb::Move) => "Making changes",
                (_, ToolVerb::Run) => "Running a command",
                (_, ToolVerb::Fetch) => "Looking up information",
                (_, ToolVerb::Think) => "Recording reasoning",
                (_, ToolVerb::Mode | ToolVerb::Tool) => "Working",
            };
        }
    }
}
