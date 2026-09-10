//! Why an owner turned a handshake away.

use serde::{Deserialize, Serialize};

use super::attach_kind::AttachKind;
use super::writer_role::WriterRole;

/// The owner's answer to a `Hello` it will not serve. Each is a fact about
/// the pairing, not about the store: nothing was opened, and the client
/// must not fall back to opening a writer of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachRefusal {
    /// The client speaks another protocol version; only an exact match is
    /// served.
    Protocol {
        /// What the owner speaks.
        served: u32,
        /// What the client asked for.
        offered: u32,
    },
    /// The client named a different project than the owner holds.
    Project {
        /// The owner's project.
        expected: String,
        /// The client's.
        offered: String,
    },
    /// The client asked for another writer role than this endpoint serves.
    Role {
        /// The endpoint's role.
        expected: WriterRole,
        /// The client's.
        offered: WriterRole,
    },
    /// The connecting process is not the owner's user, or its credentials
    /// could not be read.
    Caller(String),
    /// The owner does not offer that kind of attachment.
    Unsupported(AttachKind),
}

impl std::fmt::Display for AttachRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Protocol { served, offered } => write!(
                f,
                "owner speaks protocol {served}, client offered {offered}"
            ),
            Self::Project { expected, offered } => {
                write!(f, "owner holds project {expected}, client named {offered}")
            }
            Self::Role { expected, offered } => {
                write!(
                    f,
                    "endpoint serves the {expected:?} role, client asked for {offered:?}"
                )
            }
            Self::Caller(why) => write!(f, "caller refused: {why}"),
            Self::Unsupported(kind) => {
                write!(f, "owner does not offer {} attachment", kind.label())
            }
        }
    }
}
