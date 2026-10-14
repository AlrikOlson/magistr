//! One mutation boundary for validated workflow state and its committed provenance.

mod apply;
mod atomic_json;
mod committed;
mod error;
mod target;

pub use apply::transact;
pub use atomic_json::write_locked;
pub use committed::Committed;
pub use error::TransactionError;
pub use target::TransactionTarget;
