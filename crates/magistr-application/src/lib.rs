//! Project-scoped application contracts. Hosts supply ports; this crate never
//! opens a store, starts a runtime, interprets provider wire data or renders UI.
//! A successful command receipt certifies only the operation its use case names.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod checks;
pub mod configuration;
pub mod permissions;
pub mod projections;
pub mod sessions;
pub mod workflow;

mod application_error;
mod capability;
mod command_context;
mod command_receipt;
mod port_future;
mod project_scope;
mod recovery_action;
mod revision;

pub use application_error::ApplicationError;
pub use capability::Capability;
pub use command_context::CommandContext;
pub use command_receipt::CommandReceipt;
pub use port_future::PortFuture;
pub use project_scope::ProjectScope;
pub use recovery_action::RecoveryAction;
pub use revision::Revision;

#[cfg(test)]
mod test_support;
