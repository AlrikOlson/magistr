//! The trace-level history container and its metadata.

use serde::{Deserialize, Serialize};

use super::{branch::Branch, step::ThinkStep};

/// Per-session aggregates surfaced by snapshots and the Tauri viewer.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct HistoryMetadata {
    // u64 for the same reason as `ThinkStep.duration_ms` —
    // serde_json's default deserializer rejects u128 on the viewer side.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub total_duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub revisions_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub branches_created: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tools_used: Option<Vec<String>>,
    /// Project id resolved at the moment of the first step in this
    /// session — `<basename>-<6hex>` from cwd, or a sanitized
    /// `MAGISTR_PROJECT_ID` override. Lets the viewer group sessions
    /// by project without parsing the session id and lets the server
    /// refuse cross-project writes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub project_id: Option<String>,
}

/// One session's worth of reasoning — the canonical persistence shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct ThinkHistory {
    pub steps: Vec<ThinkStep>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub branches: Option<Vec<Branch>>,
    pub completed: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<HistoryMetadata>,
}

#[cfg(test)]
#[path = "history.tests.rs"]
mod tests;
