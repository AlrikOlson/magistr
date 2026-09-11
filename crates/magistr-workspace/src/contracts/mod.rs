//! Typed workspace outcomes shared by use cases; no serialization or transport dispatch.
mod budget;
mod envelope_version;
mod error_kind;
mod freshness;
mod port_version;
mod recovery;
mod result_set;
mod truncation;
mod truncation_reason;
mod workspace_error;
mod workspace_response;

pub use budget::Budget;
pub use envelope_version::EnvelopeVersion;
pub use error_kind::ErrorKind;
pub use freshness::Freshness;
pub use port_version::PortVersion;
pub use recovery::Recovery;
pub use result_set::ResultSet;
pub use truncation::Truncation;
pub use truncation_reason::TruncationReason;
pub use workspace_error::WorkspaceError;
pub use workspace_response::WorkspaceResponse;
