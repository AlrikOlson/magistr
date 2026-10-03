//! One MCP client lifetime across owner changes. The relay and request ledger
//! share this transport contract; each surface owns its admission and shutdown.
mod bridge;
mod frames;
mod link;
mod session;
mod session_policy;

pub(crate) use bridge::Bridge;
pub(crate) use frames::Frames;
pub(crate) use link::BridgeLink;
pub(crate) use session::{ClientSession, Inbound};
pub(crate) use session_policy::SessionPolicy;
