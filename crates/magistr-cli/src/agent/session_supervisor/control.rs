//! Narrow delegated-goal admission into the same resident session owner.

use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use super::{Command, Inner, SessionResidency, acp_error};
use crate::error::Result;

/// Host-side delegated-goal commands. Clones never own provider residency;
/// dropping the owning session closes admission for every clone.
#[derive(Clone)]
pub struct SessionControl {
    commands: mpsc::UnboundedSender<Command>,
    inner: Arc<Mutex<Inner>>,
}

impl SessionControl {
    pub(super) fn new(commands: mpsc::UnboundedSender<Command>, inner: Arc<Mutex<Inner>>) -> Self {
        Self { commands, inner }
    }

    /// Begin one negotiated goal extension while idle. The supervisor owns the
    /// request until its response, even if the caller drops the reply receiver.
    pub fn begin_goal(
        &self,
        method: String,
        params: serde_json::Value,
    ) -> Result<oneshot::Receiver<magistr_acp::Result<serde_json::Value>>> {
        let mut inner = self.inner.lock().expect("supervisor state lock poisoned");
        inner.check_admission()?;
        if inner.snapshot.active_turn.is_some() || !inner.queue.is_empty() || inner.control_busy {
            return Err(acp_error(
                "Cannot begin a goal while a turn or configuration request is active",
            ));
        }
        let turn_id = inner.next_turn;
        inner.next_turn += 1;
        inner.snapshot.active_turn = Some(turn_id);
        inner.goal_turn = Some(turn_id);
        let (reply, response) = oneshot::channel();
        if self
            .commands
            .send(Command::BeginGoal {
                method,
                params,
                reply,
                turn_id,
            })
            .is_err()
        {
            inner.finish(
                SessionResidency::Failed,
                "Session owner stopped before admitting the goal",
            );
            return Err(acp_error("Session owner is no longer available"));
        }
        Ok(response)
    }

    /// Clear an unconfirmed delegated goal through its negotiated extension.
    /// This never admits another turn or treats a clear request as its outcome.
    pub fn clear_goal(&self, method: String, params: serde_json::Value) -> Result<()> {
        let inner = self.inner.lock().expect("supervisor state lock poisoned");
        if inner.closing || inner.snapshot.residency != SessionResidency::Resident {
            return Err(acp_error("Session owner is no longer available"));
        }
        self.commands
            .send(Command::ClearGoal { method, params })
            .map_err(|_| acp_error("Session owner is no longer available"))
    }
}
