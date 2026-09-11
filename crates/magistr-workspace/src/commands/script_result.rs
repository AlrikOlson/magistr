use serde::Serialize;

use super::ScriptStatus;

/// The script row of the result envelope; write rows follow when queued.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptResult {
    pub status: ScriptStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub fuel_used: u64,
    pub rounds: u32,
    pub host_calls: u32,
    pub queued_writes: usize,
}
