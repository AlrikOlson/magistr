use serde::{Deserialize, Serialize};

use crate::cost::CostEvent;
use crate::plan::PlanEvent;
use crate::session::SessionEvent;

use super::workflow_event::WorkflowEvent;

/// Unified event type wrapping all domain sub-events.
///
/// Consumers subscribe to a single event stream and match on the variant
/// they care about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum EngineEvent {
    /// Session lifecycle event.
    Session(SessionEvent),
    /// Workflow phase/iteration event.
    Workflow(WorkflowEvent),
    /// Cost/budget event.
    Cost(CostEvent),
    /// Plan progress event.
    Plan(PlanEvent),
}
