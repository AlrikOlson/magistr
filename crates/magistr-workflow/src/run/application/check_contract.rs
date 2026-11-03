use serde::{Deserialize, Serialize};

use crate::run::domain::check::CheckType;

/// One check the project requires before a task may complete: its name, the
/// exact command a server must have run for it, and the kind of check it
/// records as. Supplied by the host's evidence capability from the project's
/// own check configuration, never derived inside the workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckContract {
    pub name: String,
    pub command: String,
    pub check_type: CheckType,
}
