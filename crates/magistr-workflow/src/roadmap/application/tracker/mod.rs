//! Tracker setup policy shared by transports, with explicit network capability.
mod error;
mod local;
mod outcome;
mod phase;
mod probe;
mod report;
mod request;
mod service;
mod status;
pub use error::TrackerSetupError;
pub use local::setup_local;
pub use outcome::SetupOutcome;
pub use phase::TargetPhase;
pub use probe::setup_probe;
pub use report::TrackerSetupReport;
pub use request::SetupRequest;
pub use status::TrackerStatus;
