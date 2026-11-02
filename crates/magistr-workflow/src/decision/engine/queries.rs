//! Validated complete reads. A request selects its log, never the last writer.
use super::ReasoningServer;
use crate::decision::{
    config::namespace_log_id,
    domain::{Log, Note},
    persistence::PersistedProject,
};
use magistr_workflow_contracts::{DecisionData, Observed, ReadError, ReadErrorKind, ReadSource};

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

    fn query_key(&self, log: Option<&str>) -> Result<String, ReadError> {
        if log.is_some_and(|id| id.trim().is_empty()) {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "A named reasoning log must have a name.",
            ));
        }
        let key = namespace_log_id(&self.project_id, log.unwrap_or(&self.project_id));
        self.persistence.validate_log_id(&key)?;
        Ok(key)
    }

    pub fn observe_logs(&self) -> Result<Observed<DecisionData>, ReadError> {
        Ok(self.observe_project()?.map(|project| DecisionData {
            logs: project.logs.into_values().collect(),
        }))
    }

    /// Missing history is None; failure cannot masquerade as an empty history.
    pub fn observe_log(&self, log: Option<&str>) -> Result<Observed<Option<Log>>, ReadError> {
        let key = self.query_key(log)?;
        Ok(self
            .observe_project()?
            .map(|mut project| project.logs.remove(&key)))
    }

    /// One globally numbered note, preserving its authored log and record identity.
    pub fn observe_note(
        &self,
        number: u32,
        latest: bool,
    ) -> Result<Observed<Option<Note>>, ReadError> {
        let read = self.observe_project()?;
        let lookup = |number| {
            read.value
                .logs
                .values()
                .flat_map(|history| &history.notes)
                .find(|note| note.note == number)
        };
        let mut note = lookup(number);
        let mut visited = std::collections::HashSet::new();
        while let Some(next) = note.filter(|_| latest).and_then(|note| note.revised_by) {
            if !visited.insert(next) {
                return Err(ReadError::new(
                    ReadErrorKind::Identity,
                    "The reasoning revision chain contains a cycle.",
                ));
            }
            note = Some(lookup(next).ok_or_else(|| {
                ReadError::new(
                    ReadErrorKind::Identity,
                    "A reasoning revision points to a missing note.",
                )
            })?);
        }
        Ok(Observed {
            value: note.cloned(),
            source: read.source,
        })
    }

    /// An untrimmed analysis view from one read; no store or selection is mutated.
    pub(crate) fn analysis(&self, log: Option<&str>) -> Result<Observed<Self>, ReadError> {
        let key = self.query_key(log)?;
        let read = self.observe_project()?;
        Ok(self.analysis_from(read, key))
    }

    pub(crate) fn analysis_for_note(&self, number: u32) -> Result<Observed<Self>, ReadError> {
        let read = self.observe_project()?;
        let key = read
            .value
            .logs
            .iter()
            .find(|(_, history)| history.notes.iter().any(|note| note.note == number))
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
