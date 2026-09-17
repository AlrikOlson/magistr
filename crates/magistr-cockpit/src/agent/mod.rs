//! The interface's session relay and tool-title presentation.
//! Shared observation and policy types live in magistr_application::sessions;
//! the CLI host translates provider data before the relay receives it.

pub mod session;
pub mod session_jobs;
pub mod tool_title;

pub use session::spawn_session;
pub use session_jobs::SessionJobs;
pub use tool_title::ToolTitle;
