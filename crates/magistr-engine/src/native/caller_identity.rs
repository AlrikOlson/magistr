//! Who is attaching.

use serde::{Deserialize, Serialize};

/// The attaching process as it names itself in the handshake: its pid and
/// the surface it is (`cockpit`, `mcp-serve`, ...). Declared, not proven —
/// the proof is the socket's peer credential, which the owner checks
/// against its own uid before it reads a byte of this. The identity is
/// kept for the owner's diagnostics and refusals, not for authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallerIdentity {
    /// The attaching process.
    pub pid: u32,
    /// Which surface it is.
    pub surface: String,
}

impl CallerIdentity {
    /// This process, attaching as `surface`.
    #[must_use]
    pub fn this_process(surface: impl Into<String>) -> Self {
        Self {
            pid: std::process::id(),
            surface: surface.into(),
        }
    }
}
