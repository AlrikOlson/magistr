//! Message transport and background jobs shared by cockpit hosts and surfaces.
//!
//! This crate owns channels, task dispatch and the session relay over application
//! handles and cockpit effect ports. magistr-cockpit owns semantic messages,
//! commands and job results. Hosts supply provider handles and concrete service
//! clients; each surface owns its event loop and frame scheduling.

#![deny(dead_code, unused)]
#![forbid(unsafe_code)]

pub mod channel;
mod interactive_session;
mod jobs;
mod local_session;
pub use interactive_session::{InteractiveSession, SessionPurpose};
mod session;
mod session_jobs;

pub use jobs::Jobs;
pub use session::spawn_session;
