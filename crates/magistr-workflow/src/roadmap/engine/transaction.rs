//! One locked read, candidate mutation, and durable acknowledgement per operation.
use super::{Observed, RoadmapEngine, RoadmapMutationError, editing::Edit, observation};
use crate::infra::transaction::{TransactionTarget, transact};
use magistr_workflow_contracts::ReadSource;

impl RoadmapEngine {
    pub(crate) fn mutate<R>(
        &mut self,
        apply: impl FnOnce(&mut Edit<'_>) -> Result<R, String>,
    ) -> Result<Observed<R>, RoadmapMutationError> {
        let path = self
            .persistence
            .as_ref()
            .filter(|p| p.enabled())
            .map(|p| p.path_for(&self.project_id));
        let lock_path = path.as_ref().map(|path| path.with_extension("json.lock"));
        let target = match (&path, &lock_path) {
            (Some(path), Some(lock_path)) => TransactionTarget::File { path, lock_path },
            _ => TransactionTarget::Memory(&self.roadmap),
        };
        let result = transact(
            target,
            || Self::empty_roadmap(&self.project_id),
            |state| observation::validate(state, &self.project_id),
            |state| {
                let mut edit = Edit {
                    roadmap: state,
                    project_id: self.project_id.clone(),
                    inherit_opt_in: self.inherit_opt_in.clone(),
                    events: Vec::new(),
                };
                let value = apply(&mut edit)?;
                Ok((value, edit.events))
            },
        );
        match result {
            Ok(committed) => {
                self.roadmap = committed.state;
                self.stamp = match committed.source {
                    ReadSource::File(stamp) => Some(stamp),
                    _ => None,
                };
                self.publish_committed(committed.value.1);
                Ok(Observed {
                    value: committed.value.0,
                    source: committed.source,
                })
            }
            Err(error) => {
                if matches!(error, crate::infra::transaction::TransactionError::Store(_)) {
                    self.stamp = None;
                }
                Err(error.into())
            }
        }
    }
}
