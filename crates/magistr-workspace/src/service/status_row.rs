use std::path::Path;

use serde::Serialize;

use crate::{
    contracts::{Budget, EnvelopeVersion, Freshness, ResultSet, WorkspaceResponse},
    diagnostics::EnvelopeVersions,
    index::watch::NetworkDetection,
};

/// The retained `workspace_status` result: deterministic for a fixed
/// workspace and environment — no pids, generations or timestamps — so
/// fresh processes answer byte-identically (the retained statelessness
/// gate). Degradations ride the envelope notes, schema-free.
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
}

impl StatusRow {
    #[must_use]
    pub fn new(
        root: &Path,
        detection: &NetworkDetection,
        applied_journal_mode: impl Into<String>,
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
        }
    }

    /// The retained status envelope: warm, one token requested and used.
    #[must_use]
    pub fn respond(self, notes: Vec<String>) -> WorkspaceResponse<Self> {
        WorkspaceResponse::Success {
            version: EnvelopeVersion::LATEST,
            freshness: Freshness::Warm,
            budget: Budget {
                requested: 1,
                used: 1,
            },
            results: ResultSet::complete(vec![self]),
            notes: (!notes.is_empty()).then_some(notes),
        }
    }
}

#[cfg(test)]
#[path = "status_row.tests.rs"]
mod tests;
