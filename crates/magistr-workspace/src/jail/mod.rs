//! Workspace path admission and owner-managed key material.
//!
//! Admission is a policy decision, not atomic descriptor-based confinement.
//! The transaction owner must revalidate immediately before filesystem effects.

mod jail_policy;
mod key_error;
mod resolve_path;
mod workspace_key;
mod workspace_key_store;

pub use jail_policy::JailPolicy;
pub use key_error::KeyError;
pub use workspace_key::WorkspaceKey;
pub use workspace_key_store::WorkspaceKeyStore;
