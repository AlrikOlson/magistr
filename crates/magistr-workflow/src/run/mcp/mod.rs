pub mod check_step_output;
pub mod handlers;
pub mod service;
mod step_result;

pub use handlers::CHECK_TIMEOUT_SECS;

/// The direct step verbs, served under their own names: they name what they
/// act on, so they carry no family prefix.
pub const STEP_VERBS: [&str; 5] = [
    "arrange_steps",
    "start_step",
    "record_progress",
    "check_step",
    "finish_step",
];
pub use service::RunService;
