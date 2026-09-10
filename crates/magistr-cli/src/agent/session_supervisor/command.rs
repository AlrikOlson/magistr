//! Commands serialized by the resident session owner.

use agent_client_protocol::schema::v1::{
    SessionConfigOption, SessionConfigOptionValue, SessionInfo,
};
use tokio::sync::oneshot;

use crate::error::Result;

pub(super) enum Command {
    Wake,
    Cancel {
        turn_id: Option<u64>,
    },
    Terminate,
    Detach,
    BeginGoal {
        method: String,
        params: serde_json::Value,
        reply: oneshot::Sender<magistr_acp::Result<serde_json::Value>>,
        turn_id: u64,
    },
    ClearGoal {
        method: String,
        params: serde_json::Value,
    },
    Permission {
        id: u64,
        option_id: String,
    },
    Constraint {
        id: u64,
        condition: String,
    },
    Delegate(String),
    KillTerminal(String),
    List {
        reply: oneshot::Sender<Result<Vec<SessionInfo>>>,
    },
    Config {
        config_id: String,
        value: SessionConfigOptionValue,
        reply: Option<oneshot::Sender<Result<Vec<SessionConfigOption>>>>,
    },
}
