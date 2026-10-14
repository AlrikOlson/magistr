use crate::roadmap::application::RoadmapError;
use crate::roadmap::engine::RoadmapMutationError;
use crate::tracker::config::TrackerConfigError;
use magistr_workflow_contracts::WriteError;

/// A setup may save configuration or create a remote destination before a later save fails.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct TrackerSetupError {
    pub kind: &'static str,
    pub message: String,
    pub store: Option<WriteError>,
    pub effect: &'static str,
}

impl TrackerSetupError {
    pub(crate) fn new(kind: &'static str, message: impl ToString) -> Self {
        Self {
            kind,
            message: message.to_string(),
            store: None,
            effect: "none",
        }
    }

    pub(crate) fn partial(mut self) -> Self {
        self.effect = "unknown";
        self
    }

    pub(crate) fn roadmap(error: RoadmapError) -> Self {
        Self {
            kind: error.kind(),
            message: error.to_string(),
            store: error.store_error().cloned(),
            effect: error.effect(),
        }
    }

    pub(crate) fn mutation(error: RoadmapMutationError) -> Self {
        Self::roadmap(RoadmapError::invalid_state(error))
    }

    pub(crate) fn config(error: TrackerConfigError) -> Self {
        match error {
            TrackerConfigError::Store(store) => Self {
                kind: "store_unavailable",
                message: store.to_string(),
                effect: store.effect(),
                store: Some(store),
            },
            TrackerConfigError::Invalid(message) => Self::new("invalid_args", message),
        }
    }
}
