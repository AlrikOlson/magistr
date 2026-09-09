//! Agent role specialization — re-exports from `magistr-core`.
//!
//! The canonical definitions now live in `magistr_core::role`. This module
//! re-exports them so that all existing `crate::agent::multi::role::*` paths
//! continue to work.

pub use magistr_core::role::{RoleConstraints, SpecializedRole};

#[cfg(test)]
#[path = "role.tests.rs"]
mod tests;
