//! What a checkpoint command asked, in the words a refusal names.

use crate::cockpit::CheckpointCmd;

/// The ask behind `cmd` — `save cp-4`, `restore cp-2`, `fork cp-2`,
/// `merge cp-3` — as
/// [`CheckpointEvent::Failed`](crate::cockpit::CheckpointEvent::Failed)
/// carries it, beside the checkpoint it named.
#[must_use]
pub fn describe(cmd: &CheckpointCmd) -> (String, String) {
    let (verb, id) = match cmd {
        CheckpointCmd::Save { id, .. } => ("save", id),
        CheckpointCmd::Restore { id } => ("restore", id),
        CheckpointCmd::Fork { id, .. } => ("fork", id),
        CheckpointCmd::Merge { id } => ("merge", id),
    };
    (id.clone(), format!("{verb} {id}"))
}
