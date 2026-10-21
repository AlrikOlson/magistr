//! A bounded check mutation response. Full receipts remain queryable by ID.
use crate::ship::application::{CheckExecution, CheckSettlement, SourceFreshness};
use crate::ship::domain::check::Check;
use magistr_workflow_contracts::Observed;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
pub(crate) struct ReceiptSummary {
    pub id: String,
    pub task_id: String,
    pub attempt: u64,
    pub execution: Option<CheckExecution>,
    pub freshness: Option<SourceFreshness>,
    pub source_revision: Option<String>,
    pub duration_ms: Option<u64>,
}
#[derive(Serialize, JsonSchema)]
pub(crate) struct CheckResult {
    pub settlement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<Observed<Check>>,
    pub receipt: ReceiptSummary,
}
impl From<CheckSettlement> for CheckResult {
    fn from(settled: CheckSettlement) -> Self {
        let record = &settled.receipt().value;
        let receipt = ReceiptSummary {
            id: record.id.clone(),
            task_id: record.target.task_id.clone(),
            attempt: record.attempt,
            execution: record.execution().cloned(),
            freshness: record.freshness.clone(),
            source_revision: record.source.as_ref().map(|source| source.revision.clone()),
            duration_ms: record.duration_ms,
        };
        match settled {
            CheckSettlement::Applied { check, .. } => Self {
                settlement: "applied".into(),
                check: Some(check),
                receipt,
            },
            CheckSettlement::Stale { .. } => Self {
                settlement: "stale".into(),
                check: None,
                receipt,
            },
        }
    }
}
