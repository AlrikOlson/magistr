//! The line protocol between an owner endpoint and its attached clients.
//!
//! One JSON object per line, both ways, over a Unix domain socket. The
//! client opens with a [`Hello`]; the owner answers with a [`Greeting`];
//! then, for a commands attachment, the client sends [`ClientFrame`]s and
//! the owner answers each command with a [`ReplyFrame`] carrying the same
//! id. An MCP attachment hands the socket to the owner's rmcp session as
//! soon as the greeting is written, so neither side sends another frame. A
//! probe attachment is answered with the greeting, then one
//! [`OwnerReport`](super::OwnerReport) line, then EOF.

use serde::{Deserialize, Serialize};

use super::attach_kind::AttachKind;
#[cfg(unix)]
use super::attach_refusal::AttachRefusal;
use super::build_identity::BuildIdentity;
use super::caller_identity::CallerIdentity;
use super::owner_command::OwnerCommand;
use super::owner_generation::OwnerGeneration;
use super::owner_identity::OwnerIdentity;
#[cfg(unix)]
use super::owner_reply::OwnerReply;

/// The protocol this binary speaks. Exact match only — during development
/// every change to the frames is a new version, and an older or newer
/// client is refused rather than guessed at. The owner reads this field
/// before the rest of the `Hello`, so a client of another version is
/// answered with [`AttachRefusal::Protocol`] whatever else its frame
/// carries; that refusal has kept the same shape since version 3.
///
/// History: 3 carried the package version as the build; 4 carries a
/// [`BuildIdentity`] and refuses on it exactly; 5 adds the `probe`
/// attachment, answered with a welcome and one
/// [`OwnerReport`](super::OwnerReport) line before the owner closes.
pub const OWNER_PROTOCOL_VERSION: u32 = 5;

/// The first line a client sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Hello {
    pub(crate) protocol: u32,
    /// The client's build; an owner serves only its exact build
    /// ([`BuildIdentity::admits`]), so a session never mixes two contracts
    /// across one workspace.
    pub(crate) build: BuildIdentity,
    pub(crate) identity: OwnerIdentity,
    pub(crate) attach: AttachKind,
    pub(crate) caller: CallerIdentity,
}

impl Hello {
    /// Read a client's first line, deciding on the protocol field alone
    /// before the rest of the frame is interpreted.
    ///
    /// # Errors
    /// `Err(Some(refusal))` for a well-formed line of another protocol
    /// version — the owner writes that refusal back so the client learns
    /// why; `Err(None)` for a line that is not a `Hello` of the served
    /// version at all, which is dropped.
    #[cfg(unix)]
    pub(crate) fn parse(line: &[u8]) -> Result<Self, Option<AttachRefusal>> {
        let value: serde_json::Value = serde_json::from_slice(line).map_err(|_| None)?;
        let offered = value
            .get("protocol")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(None)?;
        if offered != OWNER_PROTOCOL_VERSION {
            return Err(Some(AttachRefusal::Protocol {
                served: OWNER_PROTOCOL_VERSION,
                offered,
            }));
        }
        serde_json::from_value(value).map_err(|_| None)
    }
}

/// The owner's answer to a [`Hello`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "greeting", rename_all = "snake_case")]
#[cfg(unix)]
pub(crate) enum Greeting {
    Welcome {
        generation: OwnerGeneration,
        owner_pid: u32,
    },
    Refused(AttachRefusal),
}

/// What a commands client sends after the greeting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub(crate) enum ClientFrame {
    Command {
        id: u64,
        generation: OwnerGeneration,
        command: Box<OwnerCommand>,
    },
    Detach,
}

/// Why the owner did not carry a command out.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommandRefusal {
    StaleGeneration { current: OwnerGeneration },
    Engine(String),
}

/// The owner's answer to one command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(unix)]
pub(crate) struct ReplyFrame {
    pub(crate) id: u64,
    pub(crate) outcome: Result<OwnerReply, CommandRefusal>,
}
