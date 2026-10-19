//! Presentation-neutral session observations and effect ports used by the Rust application.
//! Window layout, text editing and accessibility belong to the desktop.
#![deny(dead_code, unused)]
#![forbid(unsafe_code)]
pub mod checkpoint;
pub mod cockpit;
pub mod engine;
pub mod harness;
pub mod job;
pub mod workspace;
