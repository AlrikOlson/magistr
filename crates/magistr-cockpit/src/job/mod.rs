//! Work that must not run on the loop thread.
//!
//! A job is a closure handed to [`Jobs::spawn_job`]; it runs on tokio's
//! blocking pool and its [`JobOutput`] comes back through the runtime's
//! inbox as [`Msg::Job`](crate::cockpit::Msg::Job), so `update` sees the
//! result as one more message and the cockpit keeps painting meanwhile.

pub mod job_id;
pub mod jobs;
pub mod output;

pub use job_id::JobId;
pub use jobs::Jobs;
pub use output::JobOutput;
