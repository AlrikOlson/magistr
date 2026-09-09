use serde::{Deserialize, Serialize};

/// One unit of work the fleet can execute: a prompt to run against a
/// project, on behalf of whoever asked for it.
///
/// The id is supplied by the submitter rather than minted here. That is
/// deliberate: a client that retries a submission it never saw the
/// answer to must not create a second job, and an id the caller chose is
/// the only thing that makes the submission idempotent without a second
/// round trip. [`WorkQueue::submit`] refuses a duplicate.
///
/// `project` is what placement matches a node's declared projects
/// against; `requested_by` is the client the stream relay's delivered
/// log belongs to, so a job's events can be attributed to the connection
/// that asked for them even after the node producing them has changed.
///
/// [`WorkQueue::submit`]: super::work_queue::WorkQueue::submit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkUnit {
    /// Stable id, chosen by the submitter.
    pub id: String,
    /// The project this work runs against.
    pub project: String,
    /// The prompt the remote agent is asked to run.
    pub prompt: String,
    /// The client that asked for this work.
    #[serde(default)]
    pub requested_by: String,
}

impl WorkUnit {
    /// A unit of work for `project`, requested by nobody in particular.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        project: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            project: project.into(),
            prompt: prompt.into(),
            requested_by: String::new(),
        }
    }

    /// Name the client this work is for (self-consuming builder).
    #[must_use]
    pub fn requested_by(mut self, client: impl Into<String>) -> Self {
        self.requested_by = client.into();
        self
    }
}

#[cfg(test)]
#[path = "work_unit.tests.rs"]
mod tests;
