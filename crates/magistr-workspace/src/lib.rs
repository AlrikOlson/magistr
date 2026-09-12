//! Native workspace engine adopted from the pinned Workbench source.
//!
//! Wraps tree-sitter (twelve bundled grammar crates), the `grep` crate
//! (ripgrep's engine as a library) and ast-grep (structural match/rewrite)
//! through ordinary Rust modules. Parsing, search, rewriting, indexing,
//! language packs and the sandbox preserve the source implementation; this
//! library has no Node addon, loader or service lifecycle. Native policy modules
//! provide configuration/path/key admission and typed response/byte contracts;
//! transport serialization is isolated in `response`. Adoption evidence
//! and the retained reference live under `subsystems/workspace/`.

pub mod commands;
pub mod config;
pub mod confirmation;
pub mod contracts;
pub mod diagnostics;
pub mod engine;
pub mod execution;
#[cfg(feature = "sandbox")]
pub mod extensions;
pub mod file_content;
pub mod index;
pub mod indexer;
pub mod jail;
pub mod journal;
#[cfg(feature = "lang-pack")]
pub mod lang_registry;
pub mod langs;
pub mod plugins;
pub mod query;
pub mod response;
#[cfg(feature = "sandbox")]
pub mod sandbox;
pub mod service;
pub mod snapshot;
mod source_language;
mod source_open;
pub mod transaction;
pub mod undo;
pub mod worker;
pub mod workspace_policy;

/// Version of the native Magistr workspace library.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
#[path = "lib.tests.rs"]
mod tests;
