//! Validated complete reads. A request selects its session, never the last writer.
use super::ReasoningServer;
use crate::think::{
    config::namespace_session_id,
    domain::{ThinkHistory, ThinkStep},
    persistence::PersistedProject,
};
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource, ThinkData};

impl ReasoningServer {
    fn observe_project(&self) -> Result<Observed<PersistedProject>, ReadError> {
        if !self.persistence.enabled() {
            return Ok(Observed {
                value: self.project.clone(),
                source: ReadSource::Memory,
            });
        }
        Ok(self
            .persistence
            .load_project()?
            .map(|project| project.unwrap_or_else(|| PersistedProject::empty(&self.project_id))))
    }

    fn query_key(&self, session: Option<&str>) -> Result<String, ReadError> {
        if session.is_some_and(|id| id.trim().is_empty()) {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "A named reasoning session must have a name.",
            ));
        }
        let key = namespace_session_id(&self.project_id, session.unwrap_or(&self.project_id));
        self.persistence.validate_session_id(&key)?;
        Ok(key)
    }

    pub fn observe_sessions(&self) -> Result<Observed<ThinkData>, ReadError> {
        Ok(self.observe_project()?.map(|project| ThinkData {
            sessions: project.histories.into_values().collect(),
        }))
    }

    /// Missing history is None; failure cannot masquerade as an empty history.
    pub fn observe_session(
        &self,
        session: Option<&str>,
    ) -> Result<Observed<Option<ThinkHistory>>, ReadError> {
        let key = self.query_key(session)?;
        Ok(self
            .observe_project()?
            .map(|mut project| project.histories.remove(&key)))
    }

    /// One globally numbered step, preserving its authored session and record identity.
    pub fn observe_step(
        &self,
        number: u32,
        latest: bool,
    ) -> Result<Observed<Option<ThinkStep>>, ReadError> {
        let read = self.observe_project()?;
        let lookup = |number| {
            read.value
                .histories
                .values()
                .flat_map(|history| &history.steps)
                .find(|step| step.step_number == number)
        };
        let mut step = lookup(number);
        let mut visited = std::collections::HashSet::new();
        while let Some(next) = step.filter(|_| latest).and_then(|step| step.revised_by) {
            if !visited.insert(next) {
                return Err(ReadError::new(
                    ReadErrorKind::Identity,
                    "The reasoning revision chain contains a cycle.",
                ));
            }
            step = Some(lookup(next).ok_or_else(|| {
                ReadError::new(
                    ReadErrorKind::Identity,
                    "A reasoning revision points to a missing step.",
                )
            })?);
        }
        Ok(Observed {
            value: step.cloned(),
            source: read.source,
        })
    }

    /// An untrimmed analysis view from one read; no store or selection is mutated.
    pub(crate) fn analysis(&self, session: Option<&str>) -> Result<Observed<Self>, ReadError> {
        let key = self.query_key(session)?;
        let read = self.observe_project()?;
        Ok(self.analysis_from(read, key))
    }

    pub(crate) fn analysis_for_step(&self, number: u32) -> Result<Observed<Self>, ReadError> {
        let read = self.observe_project()?;
        let key = read
            .value
            .histories
            .iter()
            .find(|(_, history)| history.steps.iter().any(|step| step.step_number == number))
            .map(|(key, _)| key.clone())
            .unwrap_or_else(|| self.project_id.clone());
        Ok(self.analysis_from(read, key))
    }

    fn analysis_from(&self, read: Observed<PersistedProject>, key: String) -> Observed<Self> {
        let mut view = Self::for_analysis(Self::new_history(), Default::default());
        view.config = self.config.clone();
        view.project_id = self.project_id.clone();
        view.install_project(read.value, (key != self.project_id).then_some(key), false);
        Observed {
            value: view,
            source: read.source,
        }
    }
}
