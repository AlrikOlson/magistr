use crate::plan::domain::Task;

use super::{PlanError, PlanUseCases};

impl PlanUseCases {
    pub fn next(&self) -> Result<Option<Task>, PlanError> {
        Ok(self.lock()?.next().cloned())
    }

    /// Preserve the extensible query projection; no transport response is
    /// decoded back into domain state. Optional request counts share ownership.
    pub fn status(&self) -> Result<serde_json::Value, PlanError> {
        let mut status = self.lock()?.status();
        // Never hold the plan lock while acquiring the request lock. The
        // inbox is observed before it is counted, like the plan above.
        if let Some(request) = &self.request
            && let Ok(mut request) = request.lock()
            && let Some(object) = status.as_object_mut()
        {
            if let Err(error) = request.refresh_if_stale() {
                tracing::warn!("magistr: request store not observed: {error}");
            }
            object.insert("ready_requests".into(), request.ready_count(0.6).into());
        }
        Ok(status)
    }

    pub fn get(&self, ids: &[String], fields: &[String]) -> Result<serde_json::Value, PlanError> {
        self.lock()?
            .get(ids, fields)
            .map_err(PlanError::InvalidArgs)
    }
}
