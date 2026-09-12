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
    /// A push interval was asked for, but this host runs no unattended push
    /// cadence — that belonged to the retired standalone server — so the
    /// interval was not applied. Reported rather than silently skipped.
    pub auto_push_not_applied: bool,
    pub pushed: Option<usize>,
}
