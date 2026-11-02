//! The log-level history container and its metadata.

use serde::{Deserialize, Serialize};

use super::{Note, Thread};

/// Per-log aggregates surfaced by snapshots and the Tauri viewer.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct LogMetadata {
    // u64 for the same reason as `Note.duration_ms` —
    // serde_json's default deserializer rejects u128 on the viewer side.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub total_duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub revisions_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub threads_created: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tools_used: Option<Vec<String>>,
    /// Project id resolved at the moment of the first note in this
    /// log — `<basename>-<6hex>` from cwd, or a sanitized
    /// `MAGISTR_PROJECT_ID` override. Lets the viewer group logs
    /// by project without parsing the log id and lets the server
    /// refuse cross-project writes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub project_id: Option<String>,
}

/// One log's worth of reasoning — the canonical persistence shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct Log {
    pub notes: Vec<Note>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub threads: Option<Vec<Thread>>,
    pub completed: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub log_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<LogMetadata>,
}

#[cfg(test)]
#[path = "log.tests.rs"]
mod tests;
