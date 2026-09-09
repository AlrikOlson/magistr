//! Commands from [`AcpTransport`](super::AcpTransport) to its connection task.

use std::path::PathBuf;

use agent_client_protocol::schema::v1::{
    SessionConfigOption, SessionConfigOptionValue, SessionInfo,
};
use tokio::sync::oneshot;

use crate::Result;

pub(crate) enum AcpCommand {
    /// Send one user turn.
    Prompt(String),
    /// Ask the agent to stop the turn in flight (`session/cancel`).
    Cancel,
    /// `session/list`.
    ListSessions {
        cwd: Option<PathBuf>,
        response_tx: oneshot::Sender<Result<Vec<SessionInfo>>>,
    },
    /// `session/set_config_option`.
    SetConfigOption {
        session_id: String,
        config_id: String,
        value: SessionConfigOptionValue,
        response_tx: oneshot::Sender<Result<Vec<SessionConfigOption>>>,
    },
    /// A vendor extension request — any `_`-prefixed method with JSON
    /// params, answered with the raw JSON result. Served while a prompt
    /// is in flight too, off the command loop, because an extension
    /// call may last a whole turn (the npm-era agent's `_session/goal`
    /// returns only when the goal loop it starts has ended).
    Extension {
        method: String,
        params: serde_json::Value,
        response_tx: oneshot::Sender<Result<serde_json::Value>>,
    },
    /// Shut the connection down and let the task exit.
    Close,
}
