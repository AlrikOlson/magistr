//! One checkpoint command, carried out on a port.

use super::port::CheckpointPort;
use crate::cockpit::{CheckpointCmd, CheckpointEvent};

/// Carry `cmd` out on `port` and say what happened — the event the
/// consumer posts back to the cockpit as
/// [`Msg::Checkpoint`](crate::cockpit::Msg::Checkpoint). A refusal
/// becomes [`CheckpointEvent::Failed`] naming both what was asked and
/// the checkpoint it was asked about, so a save that failed can mark
/// the row the permission card is already pointing at.
pub fn apply(port: &mut dyn CheckpointPort, cmd: &CheckpointCmd) -> CheckpointEvent {
    match cmd {
        CheckpointCmd::Save { id, reason, label } => match port.save(id, *reason, label) {
            Ok((head, verified)) => CheckpointEvent::Saved {
                id: id.clone(),
                head,
                verified,
            },
            Err(error) => failed(id, format!("save {id}"), error),
        },
        CheckpointCmd::Restore { id } => match port.restore(id) {
            Ok((iteration, severed)) => CheckpointEvent::Restored {
                id: id.clone(),
                iteration,
                severed,
            },
            Err(error) => failed(id, format!("restore {id}"), error),
        },
        CheckpointCmd::Fork { id, branch } => match port.fork(id, branch.as_deref()) {
            Ok((branch, path)) => CheckpointEvent::Forked {
                id: id.clone(),
                branch,
                path,
            },
            Err(error) => failed(id, format!("fork {id}"), error),
        },
        CheckpointCmd::Merge { id } => match port.merge(id) {
            Ok((branch, head)) => CheckpointEvent::Merged {
                id: id.clone(),
                branch,
                head,
            },
            Err(error) => failed(id, format!("merge {id}"), error),
        },
    }
}

fn failed(id: &str, what: String, error: String) -> CheckpointEvent {
    CheckpointEvent::Failed {
        id: Some(id.to_owned()),
        what,
        error,
    }
}
