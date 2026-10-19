use crate::roadmap::domain::Chunk;

use super::{RoadmapError, RoadmapUseCases};

impl RoadmapUseCases {
    pub fn next(&self) -> Result<Option<Chunk>, RoadmapError> {
        Ok(self.lock()?.next().cloned())
    }

    /// Preserve the extensible query projection; no transport response is
    /// decoded back into domain state. Optional signal counts share ownership.
    pub fn status(&self) -> Result<serde_json::Value, RoadmapError> {
        let mut status = self.lock()?.status();
        // Never hold the roadmap lock while acquiring the signal lock. The
        // inbox is observed before it is counted, like the roadmap above.
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

    pub fn get(
        &self,
        ids: &[String],
        fields: &[String],
    ) -> Result<serde_json::Value, RoadmapError> {
        self.lock()?
            .get(ids, fields)
            .map_err(RoadmapError::InvalidArgs)
    }
}
