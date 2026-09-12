//! The line protocol between an owner endpoint and its attached clients.
//!
//! One JSON object per line, both ways, over a Unix domain socket. The
//! client opens with a [`Hello`]; the owner answers with a [`Greeting`];
//! then, for a commands attachment, the client sends [`ClientFrame`]s and
//! the owner answers each command with a [`ReplyFrame`] carrying the same
//! id. An MCP attachment hands the socket to the owner's rmcp session as
//! soon as the greeting is written, so neither side sends another frame.

use serde::{Deserialize, Serialize};

use super::attach_kind::AttachKind;
use super::attach_refusal::AttachRefusal;
use super::caller_identity::CallerIdentity;
use super::owner_command::OwnerCommand;
use super::owner_generation::OwnerGeneration;
use super::owner_identity::OwnerIdentity;
use super::owner_reply::OwnerReply;

/// The protocol this binary speaks. Exact match only — during development
/// every change to the frames is a new version, and an older or newer
/// client is refused rather than guessed at.
pub const OWNER_PROTOCOL_VERSION: u32 = 3;

/// The build this binary is: an owner serves only clients of the same
/// build, so a session never mixes two versions' tool surfaces or
/// confirmation formats across one workspace. Equal package versions from
/// different checkouts are not told apart.
pub(crate) const OWNER_BUILD: &str = env!("CARGO_PKG_VERSION");

/// The first line a client sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Hello {
    pub(crate) protocol: u32,
    pub(crate) build: String,
    pub(crate) identity: OwnerIdentity,
    pub(crate) attach: AttachKind,
    pub(crate) caller: CallerIdentity,
}

/// The owner's answer to a [`Hello`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "greeting", rename_all = "snake_case")]
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
pub(crate) struct ReplyFrame {
    pub(crate) id: u64,
    pub(crate) outcome: Result<OwnerReply, CommandRefusal>,
}
