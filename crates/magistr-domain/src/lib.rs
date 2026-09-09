//! Deterministic Magistr-owned values, independent of stores, hosts and renderers.
//!
//! Effects supply identities and observations. This crate neither allocates IDs
//! from clocks/randomness nor loads project or provider state. Native workflow
//! library records stay behind the engine's native adapter.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod gate;
pub mod identity;
pub mod policy;
pub mod project;
pub mod workflow;

pub use project::ProjectContext;
