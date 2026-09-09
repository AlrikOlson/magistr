//! What a job produced.

use crate::cockpit::{BornData, GalaxyData, LauncherData, Proposal, Shelf, SurveyReport};

/// The result of a job, as `update` receives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobOutput {
    /// The job finished with this text.
    Text(String),
    /// The job gathered the launcher's card data.
    Launcher(Box<LauncherData>),
    /// The job gathered the Galaxy's projects and spend.
    Galaxy(Box<GalaxyData>),
    /// The job gathered a shipped chunk's deliverables — or wrote them
    /// as files, in which case the shelf names its bundle.
    Shelf(Box<Shelf>),
    /// The job surveyed a new project's ground (Birth F2): what
    /// `magistr init` detected and wrote.
    Survey(Box<SurveyReport>),
    /// The job drafted a new project's roadmap (Birth F4).
    Proposal(Box<Proposal>),
    /// The job wrote the accepted roadmap into the new project's store
    /// (Birth F5).
    Born(Box<BornData>),
    /// The job failed — or panicked, in which case this is the panic's
    /// message — and this is why.
    Failed(String),
}

impl JobOutput {
    /// Whether the job did not finish.
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}
