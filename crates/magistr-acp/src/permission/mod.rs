//! The permission round-trip between the agent and the application.
//!
//! The agent sends `session/request_permission` when a tool needs approval;
//! the transport forwards it as an [`AcpPermissionRequest`] carrying a oneshot
//! for the answer, so the TUI or GUI can show a prompt and reply.

mod option;
mod option_kind;
mod request;
mod response;

pub use option::AcpPermissionOption;
pub use option_kind::AcpPermissionOptionKind;
pub use request::AcpPermissionRequest;
pub use response::AcpPermissionResponse;
