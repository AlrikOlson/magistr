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
    pub auto_push: Option<String>,
    /// Auto-push was wanted but there was no think-and-ship entry to merge into,
    /// i.e. `init` has not run here. Reported rather than silently skipped.
    pub auto_push_no_entry: bool,
    /// WHERE auto-push was written, so the human can go look at the right file
    /// — there are up to four candidates and the answer is not obvious.
    pub auto_push_at: Option<String>,
    /// The value was ALREADY what we wanted, so nothing changed and there is
    /// nothing to reconnect for. Distinguished from a write because "set 300s"
    /// when it was already 300s claims work that did not happen.
    pub auto_push_unchanged: bool,
    /// The entry was FOUND but lives in a config we will not rewrite. Carries
    /// the one-line instruction the human needs.
    pub auto_push_manual: Option<String>,
    pub pushed: Option<usize>,
}
