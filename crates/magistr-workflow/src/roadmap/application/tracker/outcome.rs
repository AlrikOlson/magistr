/// What a setup run did locally. Nothing here describes a push: this host
/// runs no unattended tracker cadence and the setup tool sends nothing, so
/// an outcome that reported one would be a promise the host cannot keep.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SetupOutcome {
    pub target_existed: bool,
    pub target_created: bool,
    /// The provider could not be asked. NOT the same as absent — see
    /// [`super::TargetPhase::Unverifiable`].
    pub target_unverified: bool,
    pub mirroring_enabled: bool,
    pub included: Vec<String>,
    pub already_included: usize,
    pub roadmap_source: Option<magistr_workflow_contracts::ReadSource>,
}
