use serde::{Deserialize, Serialize};

/// A source location lifted from the resolver's typed WIT result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLocation {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

impl From<crate::sandbox::ResolvedLocation> for WorkerLocation {
    fn from(location: crate::sandbox::ResolvedLocation) -> Self {
        Self {
            path: location.path,
            line: location.line,
            column: location.column,
        }
    }
}
