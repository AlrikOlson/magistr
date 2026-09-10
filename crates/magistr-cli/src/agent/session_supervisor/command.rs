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
    List {
        reply: oneshot::Sender<Result<Vec<SessionInfo>>>,
    },
    Config {
        config_id: String,
        value: SessionConfigOptionValue,
        reply: Option<oneshot::Sender<Result<Vec<SessionConfigOption>>>>,
    },
}
