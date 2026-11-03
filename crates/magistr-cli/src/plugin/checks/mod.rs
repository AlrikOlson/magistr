//! Built-in check plugin implementations.
//!
//! These are native Rust implementations of check plugins that
//! don't require a subprocess. They implement the [`CheckPlugin`] trait
//! directly and can be used alongside subprocess-based check plugins.

pub mod license;
pub mod sbom;
pub mod sonarqube;
