use serde::{Deserialize, Serialize};

use crate::ship::domain::check::CheckType;

/// One gate the project requires before a chunk may complete: its name, the
/// exact command a server must have run for it, and the kind of check it
/// records as. Supplied by the host's evidence capability from the project's
/// own gate configuration, never derived inside the workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateContract {
    pub name: String,
    pub command: String,
    pub check_type: CheckType,
}
