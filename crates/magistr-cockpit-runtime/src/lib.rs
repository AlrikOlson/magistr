//! Message transport and background jobs shared by cockpit hosts and surfaces.
//!
//! This crate owns channels and task dispatch, while magistr-cockpit owns their
//! semantic messages, commands and job results. It owns no renderer, provider
//! session, concrete store or workspace client. The CLI owns session relays;
//! each surface owns its event loop and frame scheduling.

#![deny(dead_code, unused)]
#![forbid(unsafe_code)]

pub mod channel;
mod jobs;

pub use jobs::Jobs;
