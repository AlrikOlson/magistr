//! Configured gate execution, with recording evidence kept separate from exit
//! status. Process launch and native check persistence belong to the adapter.

mod check_execution;
mod check_port;
mod check_recording;
mod check_target;
mod run_check;
mod service;

pub use check_execution::CheckExecution;
pub use check_port::CheckPort;
pub use check_recording::CheckRecording;
pub use check_target::CheckTarget;
pub use run_check::RunCheck;
pub use service::Checks;
