//! Everything a host knows about a project before its session starts, as data.

use magistr_workflow_contracts::NativeSnapshot;

use super::SupervisionSeed;

/// Application-owned project identity, policy, store observations and draft.
/// This data crosses threads independently of any renderer or native window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectSeed {
    /// The project's name.
    pub name: String,
    /// The directory it lives in.
    pub dir: String,
    /// The supervision rail.
    pub supervision: SupervisionSeed,
    /// Complete workflow data shared by observation clones; writers replace it atomically.
    pub snapshot: std::sync::Arc<NativeSnapshot>,
    /// The workspace's files, for the composer's chooser.
    pub workspace_files: Vec<String>,
    pub session_address: Option<magistr_domain::identity::SessionAddress>,
    /// Bounded owner observations used when returning to a resident session.
    pub replay: Vec<super::Msg>,
    pub draft: Option<String>,
}
