//! Which kind of host admitted the project's single writer.
use serde::{Deserialize, Serialize};

/// Descriptive host identity. Both kinds contend on one project lease and
/// serve the same execution-bound command and MCP admission contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriterRole {
    /// The agent's MCP server.
    Service,
    /// An interactive or headless run host.
    Host,
}
impl WriterRole {
    /// The resource another writer already holds.
    #[must_use]
    pub const fn resource(self) -> &'static str {
        "project owner"
    }
}
