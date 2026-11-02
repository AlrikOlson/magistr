//! Validated plan reads through this engine's selected persistence owner.

use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};

use super::{Plan, PlanEngine};

impl PlanEngine {
    /// Read current stored records without changing this writer's in-memory state.
    pub fn observe(&self) -> Result<Observed<Plan>, ReadError> {
        let Some(persistence) = self.persistence.as_ref().filter(|p| p.enabled()) else {
            return Ok(Observed {
                value: self.plan.clone(),
                source: ReadSource::Memory,
            });
        };
        let read = persistence.load_stamped::<Plan>(&self.project_id)?;
        let value = read
            .value
            .unwrap_or_else(|| Self::empty_plan(&self.project_id));
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
        let changed = self.stamp != stamp || self.plan != read.value;
        self.plan = read.value;
        self.stamp = stamp;
        Ok(changed)
    }
}

#[cfg(test)]
#[path = "observation.tests.rs"]
mod tests;
pub(super) fn validate(value: &Plan, project_id: &str) -> Result<(), ReadError> {
    if value.schema_version != magistr_workflow_contracts::PLAN_SCHEMA_VERSION {
        return Err(ReadError::new(
            ReadErrorKind::Interrupted,
            "Saved work is waiting for a one-time upgrade. Start Magistr to finish it.",
        ));
    }
    if value.project_id != project_id
        || value
            .tasks
            .iter()
            .any(|task| task.project_id.as_deref() != Some(project_id))
    {
        return Err(ReadError::new(
            ReadErrorKind::Identity,
            "The saved plan could not be matched to this project.",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    if value
        .tasks
        .iter()
        .any(|task| task.id.trim().is_empty() || !ids.insert(&task.id))
    {
        return Err(ReadError::new(
            ReadErrorKind::Identity,
            "The saved plan contains incomplete or duplicate work.",
        ));
    }

    Ok(())
}
