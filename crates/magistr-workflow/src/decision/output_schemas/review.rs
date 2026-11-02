//! Output schema for `decision_review`.

use schemars::JsonSchema;
use serde::Serialize;

use crate::decision::engine::maintenance::MaintenanceReport;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewOutput {
    pub log_id: Option<String>,
    pub log_present: bool,
    pub source: magistr_workflow_contracts::ReadSource,
    pub open_hypotheses: Vec<CheckpointHypothesis>,
    pub stale_threads: Vec<CheckpointStaleThread>,
    /// One of: `"rising"`, `"falling"`, `"stable"`, `"insufficient_data"`.
    pub confidence_trend: String,
    pub revised_but_undefended: Vec<CheckpointRevisedUndefended>,
    pub refuted_chain_alerts: Vec<CheckpointRefutedChain>,
    /// Pinned-set size and rollup pressure, plus any pins the decay rule
    /// proposes releasing. The engine's own type, reused rather than mirrored —
    /// a hand-copied mirror is how the advertised schema drifts from what the
    /// tool actually returns.
    pub maintenance: MaintenanceReport,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CheckpointHypothesis {
    pub note: u32,
    pub thought_excerpt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CheckpointStaleThread {
    pub id: String,
    pub name: String,
    pub last_note: u32,
    pub notes_behind: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CheckpointRevisedUndefended {
    pub note: u32,
    pub revised_by: u32,
    pub depending_notes_unaware: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CheckpointRefutedChain {
    pub note: u32,
    pub refuted_ancestors: Vec<u32>,
}
