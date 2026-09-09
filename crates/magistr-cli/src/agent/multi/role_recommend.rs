//! Role recommendation — re-exports from `magistr-core`.
//!
//! The canonical definitions now live in `magistr_core::role_recommend`. This
//! module re-exports them so that all existing `crate::agent::multi::role_recommend::*`
//! paths continue to work.

pub use magistr_core::role_recommend::{
    recommend_roles_for_task, recommend_roles_with_description,
};

#[cfg(test)]
#[path = "role_recommend.tests.rs"]
mod tests;
