//! Built-in quality gate plugin implementations.
//!
//! These are native Rust implementations of quality gate plugins that
//! don't require a subprocess. They implement the [`GatePlugin`] trait
//! directly and can be used alongside subprocess-based gate plugins.

pub mod license;
pub mod sbom;
pub mod sonarqube;
