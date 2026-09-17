//! Canonical ship records and typed mutation events, without host effects.

mod action;
mod artifact;
mod check;
mod frame;
mod objective;
mod report;
mod task;
mod verification_mode;

pub use action::{Action, ActionType};
pub use artifact::{Artifact, ArtifactType};
pub use check::{Check, CheckType};
pub use frame::ShipFrame;
pub use objective::{Objective, ObjectiveStatus};
pub use report::{ReportRecord, TestFailure, TestResults};
pub use task::{Task, TaskStatus, TaskType};
pub use verification_mode::VerificationMode;
