use super::resolved::ResolvedWrites;
use crate::contracts::WorkspaceError;

/// A verb either resolves to writes, has nothing to change, or is refused
/// with the retained taxonomy error and its explanatory notes.
pub(super) enum Resolution {
    Writes(ResolvedWrites),
    NoOp(Vec<String>),
    Refused(WorkspaceError, Vec<String>),
}

impl Resolution {
    pub fn refuse(error: WorkspaceError) -> Self {
        Self::Refused(error, Vec::new())
    }
}
