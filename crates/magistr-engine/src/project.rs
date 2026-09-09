//! Compatibility path for the canonical project context snapshot.
//!
//! Count fields and constructor arguments now use `usize`, matching the core
//! spec parser. Existing JSON field names and numeric representation are unchanged.

pub use magistr_domain::ProjectContext;

#[cfg(test)]
#[path = "project.tests.rs"]
mod tests;
