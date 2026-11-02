//! Output schema for `decision_search`.

use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SearchOutput {
    pub query: String,
    pub match_count: u32,
    pub matches: Vec<SearchHit>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SearchHit {
    pub note: u32,
    pub log_id: Option<String>,
    pub record_id: String,
    pub purpose: String,
    /// Which field matched: one of `thought`, `outcome`, `context`,
    /// `purpose`, `rationale`, `next_action`.
    pub matched_field: String,
    pub excerpt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_by: Option<u32>,
}
