//! What the Galaxy shows, delivered by one job.

use super::galaxy_project::GalaxyProject;

/// The Galaxy's data: every project this machine knows, and the spend
/// across them against the cap — gathered off the loop thread by the
/// entry point and posted back as one
/// [`JobOutput::Galaxy`](crate::job::JobOutput::Galaxy), the way the
/// launcher's cards arrive. Money is carried in cents so the model
/// holds no floats to disagree about.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GalaxyData {
    /// Every known project, the window's own first when it is known.
    pub projects: Vec<GalaxyProject>,
    /// Spend across every project, in USD cents.
    pub spent_cents: u64,
    /// The spend cap, in USD cents, when one is set.
    pub cap_cents: Option<u64>,
    /// Where the loops stop short of the cap, in USD cents, when a stop
    /// is set — the meter's breaker tick.
    pub stop_cents: Option<u64>,
}
