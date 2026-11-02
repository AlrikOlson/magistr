//! Canonical run records and typed mutation events, without host effects.

mod artifact;
mod check;
mod frame;
mod goal;
mod progress;
mod report;
mod step;
mod verification_mode;

pub use artifact::{Artifact, ArtifactType};
pub use check::{Check, CheckType};
pub use frame::RunFrame;
pub use goal::{Goal, GoalStatus};
pub use progress::{Progress, ProgressType};
pub use report::{ReportRecord, TestFailure, TestResults};
pub use step::{Step, StepStatus, StepType};
pub use verification_mode::VerificationMode;
