//! Native workspace engine adopted from the pinned Workbench source.
//!
//! Wraps tree-sitter (twelve bundled grammar crates), the `grep` crate
//! (ripgrep's engine as a library) and ast-grep (structural match/rewrite)
//! through ordinary Rust modules. Parsing, search, rewriting, indexing,
//! language packs and the sandbox preserve the source implementation; this
//! library has no Node addon, loader or service lifecycle. Adoption evidence
//! and the retained reference live under `subsystems/workspace/`.

pub mod engine;
pub mod indexer;
#[cfg(feature = "lang-pack")]
pub mod lang_registry;
pub mod langs;
#[cfg(feature = "sandbox")]
pub mod sandbox;

/// Version of the native Magistr workspace library.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
#[path = "lib.tests.rs"]
mod tests;
