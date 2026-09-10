use super::{SetupOutcome, TargetPhase};
/// Typed result of the shared tracker setup use case.
#[derive(Debug)]
pub struct TrackerSetupReport {
    pub project_id: String,
    pub provider: String,
    pub target: String,
    pub dry_run: bool,
    pub phase: TargetPhase,
    pub outcome: SetupOutcome,
}
