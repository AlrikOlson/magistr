//! What an attaching client asks an owner for.

use serde::{Deserialize, Serialize};

/// The service a client attaches to on an owner endpoint. An owner offers
/// what its host wired: a cockpit or loop host serves typed commands, the
/// MCP service serves its rmcp session too, and a kind the owner does not
/// offer is refused at the handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachKind {
    /// Typed owner commands — [`OwnerCommand`](super::OwnerCommand) — each
    /// carried out by the owner's own writer.
    Commands,
    /// The owner's MCP session, spoken over the same socket once the
    /// handshake is done: a second `magistr mcp` proxies its stdio
    /// here.
    Mcp,
    /// A diagnostic handshake: after the welcome the owner writes one
    /// [`OwnerReport`](super::OwnerReport) line and closes. Every owner
    /// offers it; a probe is never counted among the owner's sessions.
    Probe,
}

impl AttachKind {
    /// The word the handshake and its refusals use.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Commands => "commands",
            Self::Mcp => "mcp",
            Self::Probe => "probe",
        }
    }
}
