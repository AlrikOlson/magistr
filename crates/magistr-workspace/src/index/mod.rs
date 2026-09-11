//! Rebuildable native workspace cache; no source certification or workspace admission.
pub mod cache;
mod coordinator;
mod error;
mod observation;
mod options;
mod query;
mod selection;
mod service;
mod shared;
mod status;
mod validation_budget;
pub mod watch;
pub use error::IndexError;
pub use options::IndexOptions;
pub use query::IndexQuery;
pub use service::WorkspaceIndex;
pub use status::IndexStatus;
