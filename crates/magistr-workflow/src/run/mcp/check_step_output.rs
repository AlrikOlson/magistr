//! A bounded check mutation response. Full check_results remain queryable by ID.
use crate::run::application::{CheckExecution, CheckSettlement, SourceFreshness};
use crate::run::domain::check::Check;
use magistr_workflow_contracts::Observed;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
pub(crate) struct CheckResultSummary {
    pub id: String,
    pub step_id: String,
    pub attempt: u64,
    pub execution: Option<CheckExecution>,
    pub freshness: Option<SourceFreshness>,
    pub source_revision: Option<String>,
    pub duration_ms: Option<u64>,
}
#[derive(Serialize, JsonSchema)]
pub(crate) struct CheckStepOutput {
    pub settlement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<Observed<Check>>,
    pub check_result: CheckResultSummary,
}
impl From<CheckSettlement> for CheckStepOutput {
    fn from(settled: CheckSettlement) -> Self {
        let record = &settled.check_result().value;
        let check_result = CheckResultSummary {
            id: record.id.clone(),
            step_id: record.target.step_id.clone(),
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
                check_result,
            },
            CheckSettlement::Stale { .. } => Self {
                settlement: "stale".into(),
                check: None,
                check_result,
            },
        }
    }
}
