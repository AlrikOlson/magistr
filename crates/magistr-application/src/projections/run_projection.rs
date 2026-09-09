use magistr_domain::{identity::RunId, workflow::WorkflowPhase};

use crate::Revision;

use super::{EventSequence, OwnerGeneration, ProjectionFreshness};

/// Minimal cross-host run observation. Optional cursors mean unavailable
/// evidence, never zero; a phase is absent before workflow observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunProjection {
    pub run: RunId,
    pub revision: Revision,
    pub owner_generation: Option<OwnerGeneration>,
    pub event_sequence: Option<EventSequence>,
    pub freshness: ProjectionFreshness,
    pub phase: Option<WorkflowPhase>,
}
