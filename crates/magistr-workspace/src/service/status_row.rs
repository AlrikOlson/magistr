use std::path::Path;

use serde::Serialize;

use crate::{
    contracts::{Budget, EnvelopeVersion, Freshness, ResultSet, WorkspaceResponse},
    diagnostics::{EnvelopeVersions, IndexHealth, WorkerHealth},
    index::{IndexStatus, watch::NetworkDetection},
    query::budget,
    worker::WorkerStatus,
};

/// One observation of the admitted workspace and its owned runtime.
/// Worker and index snapshots describe current health, not source completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusRow {
    pub root: String,
    /// The mode the filesystem detector chose.
    pub journal_mode: &'static str,
    /// The mode SQLite reported after the writer applied it: a measured fact.
    pub applied_journal_mode: String,
    pub network_fs: bool,
    pub detection_method: String,
    pub envelope: EnvelopeVersions,
    pub worker: WorkerHealth,
    pub index: IndexHealth,
}

impl StatusRow {
    #[must_use]
    pub fn new(
        root: &Path,
        detection: &NetworkDetection,
        applied_journal_mode: impl Into<String>,
        worker: WorkerStatus,
        index: IndexStatus,
    ) -> Self {
        Self {
            root: root.display().to_string(),
            journal_mode: detection.journal_mode(),
            applied_journal_mode: applied_journal_mode.into(),
            network_fs: detection.network_fs,
            detection_method: detection.method.clone(),
            envelope: EnvelopeVersions {
                min: EnvelopeVersion::MIN.get(),
                latest: EnvelopeVersion::LATEST.get(),
            },
            worker: worker.into(),
            index: index.into(),
        }
    }

    /// Include runtime degradations and measure the single observation.
    #[must_use]
    pub fn respond(self, mut notes: Vec<String>) -> WorkspaceResponse<Self> {
        if self.worker.stopped {
            notes.push(
                "worker stopped: parsing and structural queries are unavailable until it restarts"
                    .into(),
            );
        } else if self.worker.consecutive_failures > 0 {
            notes.push(format!(
                "worker restarting after {} consecutive failure(s)",
                self.worker.consecutive_failures
            ));
        }
        if self.worker.quarantined > 0 {
            notes.push(format!(
                "{} input(s) blocked after repeated worker failures; fix the affected input before retrying",
                self.worker.quarantined
            ));
        }
        notes.extend(self.index.notes.iter().cloned());
        if let Some(error) = &self.index.last_error {
            notes.push(format!("index: {error}"));
        }
        let used = match budget::tokens(&self) {
            Ok(used) => used,
            Err(error) => return budget::failure(error),
        };
        WorkspaceResponse::Success {
            version: EnvelopeVersion::LATEST,
            freshness: Freshness::Warm,
            budget: Budget {
                requested: used,
                used,
            },
            results: ResultSet::complete(vec![self]),
            notes: (!notes.is_empty()).then_some(notes),
        }
    }
}

#[cfg(test)]
#[path = "status_row.tests.rs"]
mod tests;
