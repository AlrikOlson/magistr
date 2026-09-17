use magistr_application::effects::{SourceRefusal, SourceRoute};
use magistr_workspace::{contracts::WorkspaceError, service::WorkspaceOpenError};

use super::HostedWorkspace;
use crate::config::inputs::ProjectInputs;

/// What a source-writing host got when it tried to own the checkout for its
/// run — the one composition every such host shares (`magistr run` on the
/// cockpit, headless and plain, the cockpit, plain and headless loops, the
/// roadmap generator and the init interview). Either this process admitted
/// the workspace and the session's authorized writes take its pipeline, or
/// it holds the reason and the session's writes are refused. There is no
/// direct-write fallback: a held lock, a marker that needs migration, a
/// configuration the policy refused, durable data from a newer binary or a
/// store that failed to open all end as a [`SourceRefusal`], and the host
/// never attaches to another owner to write through it.
pub enum WorkspaceAdmission {
    /// This process owns the workspace for the run.
    Admitted(HostedWorkspace),
    /// This process admitted nothing; every source write is refused with this.
    Refused(SourceRefusal),
}

impl WorkspaceAdmission {
    /// Try to admit the captured project's root in this process the way
    /// [`HostedWorkspace::admit`] does; a refusal is an answer, never an error.
    pub async fn admit(inputs: &ProjectInputs) -> Self {
        Self::from_outcome(HostedWorkspace::admit(inputs).await)
    }

    /// As [`Self::admit`] on explicit open options — the seam a test uses to
    /// name the real worker binary.
    pub async fn admit_with(options: magistr_workspace::service::WorkspaceOpenOptions) -> Self {
        Self::from_outcome(HostedWorkspace::admit_with(options).await)
    }

    fn from_outcome(outcome: Result<Option<HostedWorkspace>, WorkspaceOpenError>) -> Self {
        match outcome {
            Ok(Some(workspace)) => Self::Admitted(workspace),
            Ok(None) => Self::Refused(SourceRefusal::OwnerElsewhere),
            Err(error) => Self::Refused(SourceRefusal::NotAdmitted(error.to_string())),
        }
    }

    /// The admitted workspace, when this process owns one.
    #[must_use]
    pub fn hosted(&self) -> Option<&HostedWorkspace> {
        match self {
            Self::Admitted(workspace) => Some(workspace),
            Self::Refused(_) => None,
        }
    }

    /// The route the run's sessions take for their authorized source writes:
    /// a writer over the admitted service, or the refusal. A service that
    /// refuses to mint a journal session is a refusal too, in its own words.
    #[must_use]
    pub fn source_route(&self) -> SourceRoute {
        match self {
            Self::Admitted(workspace) => match workspace.source_writer() {
                Ok(writer) => SourceRoute::Admitted(writer),
                Err(error) => SourceRoute::Refused(SourceRefusal::NotAdmitted(format!(
                    "workspace session refused: {error}"
                ))),
            },
            Self::Refused(refusal) => SourceRoute::Refused(refusal.clone()),
        }
    }

    /// One line for the normal terminal saying what the run's writes will do.
    #[must_use]
    pub fn report(&self) -> String {
        match self {
            Self::Admitted(workspace) => format!(
                "magistr: workspace admitted (generation {}); agent source writes run through its pipeline",
                workspace.generation()
            ),
            Self::Refused(SourceRefusal::OwnerElsewhere) => {
                "magistr: another owner holds the workspace; agent source writes are refused (no attachment for writing, no second owner)".to_owned()
            }
            Self::Refused(SourceRefusal::NotAdmitted(reason)) => {
                format!("magistr: workspace not admitted ({reason}); agent source writes are refused")
            }
            Self::Refused(SourceRefusal::NoAuthority) => {
                "magistr: this session has no source-write authority; agent source writes are refused".to_owned()
            }
        }
    }

    /// Release what was admitted; a refusal held nothing.
    ///
    /// # Errors
    /// As [`HostedWorkspace::shutdown`].
    pub async fn shutdown(self) -> Result<(), WorkspaceError> {
        match self {
            Self::Admitted(workspace) => workspace.shutdown().await,
            Self::Refused(_) => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "workspace_admission.tests.rs"]
mod tests;
