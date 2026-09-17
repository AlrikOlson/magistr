//! Results of background work, delivered as semantic model messages.
//!
//! The runtime assigns a [`JobId`] and posts its [`JobOutput`] through
//! [`Msg::Job`](crate::cockpit::Msg::Job). Task execution belongs to the host.

pub mod job_id;
pub mod output;

pub use job_id::JobId;
pub use output::JobOutput;
