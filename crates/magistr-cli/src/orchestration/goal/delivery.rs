//! One way of handing the goal to the agent.

use super::delivery_kind::DeliveryKind;

/// One way of handing the goal to the agent, once a session has been
/// observed: the extension's advertised method, or the discovered
/// command's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// `_session/goal` — the method string from the advertisement, the
    /// session id the params name.
    Extension {
        /// The advertised control method.
        method: String,
        /// The session the goal is set on.
        session_id: String,
    },
    /// The discovered goal command, sent as the turn
    /// [`AgentGoal::command_turn`](super::agent_goal::AgentGoal::command_turn) composes.
    Command {
        /// The command's advertised name.
        name: String,
    },
}

impl Delivery {
    /// Which kind this is — what the driver remembers across
    /// iterations, since ids and methods are per-session.
    #[must_use]
    pub const fn kind(&self) -> DeliveryKind {
        match self {
            Self::Extension { .. } => DeliveryKind::Extension,
            Self::Command { .. } => DeliveryKind::Command,
        }
    }

    /// The word the stage and the report use.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        self.kind().label()
    }
}
