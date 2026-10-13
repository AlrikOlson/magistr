//! Everything a host knows about a project before its session starts, as data.

use magistr_workflow_contracts::NativeSnapshot;

use super::{Model, SupervisionSeed};

/// A hosted project as its host seeds it: the identity, the header's line,
/// the supervision rail, the store's snapshot and the workspace's files.
///
/// A [`Model`] cannot cross threads (its highlighter state is not `Send`),
/// so a host that opens a project off the surface's thread sends this and
/// the surface builds the model from an addressed [`Msg::SessionFrame`](super::Msg::SessionFrame).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectSeed {
    /// The project's name.
    pub name: String,
    /// The directory it lives in.
    pub dir: String,
    /// The header's session line.
    pub title: String,
    /// Whether the diagnostics chrome shows.
    pub debug: bool,
    /// The supervision rail.
    pub supervision: SupervisionSeed,
    /// The store as it stood when the host opened it.
    pub snapshot: NativeSnapshot,
    /// The workspace's files, for the composer's chooser.
    pub workspace_files: Vec<String>,
    pub session_address: Option<magistr_domain::identity::SessionAddress>,
    /// Bounded owner observations used when returning to a resident session.
    pub replay: Vec<super::Msg>,
    pub draft: Option<String>,
}

impl ProjectSeed {
    /// `model` as a session hosted on this project.
    #[must_use]
    pub fn seed(self, model: Model) -> Model {
        let model = model
            .with_hosted_work()
            .with_project(self.name, self.dir)
            .with_title(self.title)
            .with_debug(self.debug);
        let mut model = self
            .supervision
            .seed(model)
            .with_snapshot(&self.snapshot)
            .with_workspace_files(self.workspace_files);
        model.session_address = self.session_address;
        for message in self.replay {
            let _ = super::update(&mut model, message);
        }
        if let Some(draft) = self.draft {
            let _ = super::update(&mut model, super::Msg::PromptReplace(draft));
        }
        model
    }
}

#[cfg(test)]
#[path = "project_seed.tests.rs"]
mod tests;
