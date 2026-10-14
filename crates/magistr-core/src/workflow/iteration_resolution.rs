//! Explicit operator settlement changes one dispatch without recovering other work.

use std::num::NonZeroU32;
use std::path::Path;

use super::{IterationState, loop_state_path};
use crate::durable_write::durable_write;
use crate::error::{MagistrError, Result};
use crate::workflow::loop_lease::LoopLease;
use crate::workflow::process_alive::is_process_alive;
use crate::workflow::runtime_handle::RuntimeHandle;
use magistr_domain::identity::{AttemptId, CommandId};
use magistr_domain::workflow::OperationOutcome;

impl IterationState {
    /// Record the operator's observed outcome. The caller must explicitly confirm
    /// that any provider not identified by a saved handle has stopped.
    /// The loop lock excludes current owners; the PID check also catches older clients.
    pub fn resolve_dispatch(
        project_dir: &Path,
        run: &str,
        command: &str,
        number: NonZeroU32,
        outcome: OperationOutcome,
        reason: &str,
    ) -> Result<Self> {
        let _lease = LoopLease::acquire(project_dir)?;
        let mut state = Self::read(project_dir)?
            .ok_or_else(|| MagistrError::Workflow("There is no saved loop to resolve.".into()))?;
        if state.run_id.local() != run {
            return Err(MagistrError::Workflow(
                "The saved run has changed. Inspect it before trying again.".into(),
            ));
        }
        if state.run_process().and_then(is_process_alive) == Some(true) {
            return Err(MagistrError::Workflow(
                "The saved loop's process is still running. Stop it before resolving work.".into(),
            ));
        }
        let command = CommandId::new(state.run_id.clone(), command)
            .map_err(|error| MagistrError::Workflow(error.to_string()))?;
        let attempt = AttemptId::new(command, number);
        let record = state.operations.record(&attempt).ok_or_else(|| {
            MagistrError::Workflow("That attempt is not in the saved run.".into())
        })?;
        if record
            .external
            .as_deref()
            .and_then(RuntimeHandle::parse)
            .is_some_and(|handle| handle.is_live() == Some(true))
        {
            return Err(MagistrError::Workflow(
                "The agent is still running. Stop it before resolving this attempt.".into(),
            ));
        }
        state
            .operations
            .resolve_dispatch(&attempt, outcome, reason)
            .map_err(|error| MagistrError::Workflow(error.to_string()))?;
        let path = loop_state_path(project_dir);
        let json = serde_json::to_vec_pretty(&state)
            .map_err(|error| MagistrError::parse("JSON", "saved loop", error))?;
        durable_write(&path, &json).map_err(|error| MagistrError::io(&path, error))?;
        Ok(state)
    }
}
