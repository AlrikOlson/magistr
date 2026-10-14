//! Validated roadmap reads through this engine's selected persistence owner.

use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};

use super::{Roadmap, RoadmapEngine};

impl RoadmapEngine {
    /// Read current stored records without changing this writer's in-memory state.
    pub fn observe(&self) -> Result<Observed<Roadmap>, ReadError> {
        let Some(persistence) = self.persistence.as_ref().filter(|p| p.enabled()) else {
            return Ok(Observed {
                value: self.roadmap.clone(),
                source: ReadSource::Memory,
            });
        };
        let read = persistence.load_stamped::<Roadmap>(&self.project_id)?;
        let value = read
            .value
            .unwrap_or_else(|| Self::empty_roadmap(&self.project_id));
        validate(&value, &self.project_id)?;
        Ok(Observed {
            value,
            source: read.source,
        })
    }

    pub(crate) fn refresh_observed(&mut self) -> Result<bool, ReadError> {
        let read = match self.observe() {
            Ok(read) => read,
            Err(error) => {
                self.stamp = None;
                return Err(error);
            }
        };
        let stamp = match read.source {
            ReadSource::Memory => return Ok(false),
            ReadSource::Missing => None,
            ReadSource::File(stamp) => Some(stamp),
        };
        let changed = self.stamp != stamp || self.roadmap != read.value;
        self.roadmap = read.value;
        self.stamp = stamp;
        Ok(changed)
    }
}

#[cfg(test)]
#[path = "observation.tests.rs"]
mod tests;
pub(super) fn validate(value: &Roadmap, project_id: &str) -> Result<(), ReadError> {
    if value.project_id != project_id
        || value
            .chunks
            .iter()
            .any(|chunk| chunk.project_id.as_deref() != Some(project_id))
    {
        return Err(ReadError::new(
            ReadErrorKind::Identity,
            "The saved roadmap could not be matched to this project.",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    if value
        .chunks
        .iter()
        .any(|chunk| chunk.id.trim().is_empty() || !ids.insert(&chunk.id))
    {
        return Err(ReadError::new(
            ReadErrorKind::Identity,
            "The saved roadmap contains incomplete or duplicate work.",
        ));
    }

    Ok(())
}
