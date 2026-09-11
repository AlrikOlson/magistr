use std::path::Path;

use serde::Serialize;

use super::{EnvelopeVersions, IndexHealth, WorkerHealth};
use crate::{
    config::ResolvedConfig,
    contracts::{EnvelopeVersion, Freshness, WorkspaceResponse},
    index::{IndexStatus, watch::NetworkDetection},
    query::budget,
    worker::WorkerStatus,
};

/// Workspace health as the retained `workbench_status` clients read it:
/// root, the SQLite journal mode the filesystem detector chose, the
/// supported envelope versions, and the worker/index snapshots when the
/// composition holds them. Degradations ride the notes, schema-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub root: String,
    pub state_dir: String,
    pub journal_mode: &'static str,
    pub network_detection: NetworkDetection,
    pub envelope_versions: EnvelopeVersions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker: Option<WorkerHealth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<IndexHealth>,
    pub notes: Vec<String>,
}

impl WorkspaceStatus {
    /// Observe without opening anything: detection reads mounts, the config
    /// contributes its warnings, and the snapshots are the caller's.
    #[must_use]
    pub fn observe(
        root: &Path,
        state_dir: &Path,
        config: &ResolvedConfig,
        network_override: Option<&str>,
        worker: Option<WorkerStatus>,
        index: Option<IndexStatus>,
    ) -> Self {
        let network_detection = NetworkDetection::detect(root, network_override);
        let mut notes = config.notes();
        if network_detection.network_fs {
            notes.push(format!(
                "workspace sits on a network filesystem (via {}); SQLite runs in TRUNCATE journal mode",
                network_detection.method
            ));
        }
        let worker = worker.map(WorkerHealth::from);
        if let Some(worker) = &worker {
            if worker.stopped {
                notes.push("worker stopped: parsing and structural queries are unavailable until it restarts".into());
            } else if worker.consecutive_failures > 0 {
                notes.push(format!(
                    "worker restarting after {} consecutive failure(s); {} input(s) quarantined this session",
                    worker.consecutive_failures, worker.quarantined
                ));
            }
        }
        let index = index.map(IndexHealth::from);
        if let Some(index) = &index {
            notes.extend(index.notes.iter().cloned());
            if let Some(error) = &index.last_error {
                notes.push(format!("index: {error}"));
            }
        }
        Self {
            root: root.display().to_string(),
            state_dir: state_dir.display().to_string(),
            journal_mode: network_detection.journal_mode(),
            network_detection,
            envelope_versions: EnvelopeVersions {
                min: EnvelopeVersion::MIN.get(),
                latest: EnvelopeVersion::LATEST.get(),
            },
            worker,
            index,
            notes,
        }
    }

    /// The status as one enveloped row; its own notes travel as the
    /// envelope's notes so every degradation is visible without a schema.
    #[must_use]
    pub fn respond(self, budget: u64) -> WorkspaceResponse<Self> {
        let notes = self.notes.clone();
        budget::success(
            vec![self],
            budget,
            Freshness::Warm,
            notes,
            |_| "status".into(),
            vec!["raise budget".into()],
        )
    }
}

#[cfg(test)]
#[path = "workspace_status.tests.rs"]
mod tests;
