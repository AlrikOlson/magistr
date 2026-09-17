/// The administrative operation a change plan was computed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminOperation {
    /// Project setup: init, re-init, presets, import, selective apply and
    /// template plugins.
    Setup,
    /// Startup repair of the files magistr owns outright.
    Repair,
    /// Owned removal of a host target's recorded artifacts.
    Removal,
}

impl AdminOperation {
    /// The lower-case label machine-readable output uses.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Repair => "repair",
            Self::Removal => "removal",
        }
    }
}
