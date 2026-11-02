use crate::plan::domain::Task;

use super::{PlanError, PlanUseCases};

impl PlanUseCases {
    pub fn next(&self) -> Result<Option<Task>, PlanError> {
        Ok(self.lock()?.next().cloned())
    }

    /// Preserve the extensible query projection; no transport response is
    /// decoded back into domain state. Optional signal counts share ownership.
    pub fn status(&self) -> Result<serde_json::Value, PlanError> {
        let mut status = self.lock()?.status();
        // Never hold the plan lock while acquiring the signal lock. The
        // inbox is observed before it is counted, like the plan above.
        if let Some(signal) = &self.signal
            && let Ok(mut signal) = signal.lock()
            && let Some(object) = status.as_object_mut()
        {
            if let Err(error) = signal.refresh_if_stale() {
                tracing::warn!("magistr: signal store not observed: {error}");
            }
            object.insert("pending_signals".into(), signal.pending_count(0.6).into());
        }
        Ok(status)
    }

    pub fn get(&self, ids: &[String], fields: &[String]) -> Result<serde_json::Value, PlanError> {
        self.lock()?
            .get(ids, fields)
            .map_err(PlanError::InvalidArgs)
    }
}
