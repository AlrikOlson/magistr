//! A candidate state and deferred events, with no persistence or publication access.
mod imports;
mod proposals;
mod tasks;
mod tracker;

use super::*;
use magistr_workflow_contracts::plan::{Readiness, WaitingDependency};

pub(crate) struct Edit<'a> {
    pub(super) plan: &'a mut Plan,
    pub(super) project_id: String,
    pub(super) inherit_opt_in: Option<String>,
    pub(super) events: Vec<PlanFrame>,
}

impl Edit<'_> {
    pub(crate) fn plan(&self) -> &Plan {
        self.plan
    }
    fn record_event(&mut self, frame: PlanFrame) {
        self.events.push(frame);
    }
    fn now() -> String {
        Utc::now().to_rfc3339()
    }

    fn index_of(&self, id: &str) -> Result<usize, String> {
        self.plan
            .tasks
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| format!("task '{id}' not found"))
    }

    fn why_feature_unfit(&self, name: &str, prospective_id: Option<&str>) -> Option<String> {
        let prefixes: Vec<String> = self
            .plan
            .tasks
            .iter()
            .map(|c| region::id_prefix(&c.id).to_string())
            .chain(prospective_id.map(|id| region::id_prefix(id).to_string()))
            .collect();
        region::why_unfit(name, prefixes.iter().map(String::as_str))
    }

    fn clean_group(feature: Option<String>) -> Option<String> {
        feature
            .map(|g| g.trim().to_string())
            .filter(|g| !g.is_empty())
    }

    pub fn workstreams(&self) -> WorkstreamCatalog {
        workstream_catalog(&self.plan.tasks)
    }

    fn normalize_ref(&self, raw: &str) -> Result<String, String> {
        CrossRef::from_wire(raw)
            .map(|r| r.to_wire())
            .map_err(|e| format!("invalid cross-ref '{raw}': {e}"))
    }

    fn push_cross_ref(&mut self, id: &str, raw: &str) -> Result<(), String> {
        let normalized = self.normalize_ref(raw)?;
        let idx = self.index_of(id)?;
        let task = &mut self.plan.tasks[idx];
        if !task.refs.contains(&normalized) {
            task.refs.push(normalized);
            task.updated_at = Self::now();
        }
        Ok(())
    }
}

impl Edit<'_> {
    pub(super) fn readiness<'a>(&self, task: &'a Task) -> Readiness<'a> {
        task.readiness(|id| {
            self.plan
                .tasks
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.status)
        })
    }

    pub(super) fn waiting_reason(&self, deps: &[WaitingDependency<'_>]) -> String {
        format!(
            "Waiting for {}",
            deps.iter()
                .map(|dep| {
                    self.plan
                        .tasks
                        .iter()
                        .find(|c| c.id == dep.id)
                        .map_or("an unavailable work item", |c| c.name.as_str())
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    pub(super) fn require_ready(&self, task: &Task) -> Result<(), String> {
        let reason = match self.readiness(task) {
            Readiness::Ready => return Ok(()),
            Readiness::Waiting(reason) => reason.to_owned(),
            Readiness::Dependent(deps) => self.waiting_reason(&deps),
            Readiness::Inactive(status) => match status {
                TaskStatus::Idea => "This work item is planned for later.",
                TaskStatus::InProgress => "Work is already underway.",
                TaskStatus::Waiting => {
                    "Resolve what this work item is waiting on and return it to the work queue."
                }
                TaskStatus::Done => "This work item is already completed.",
                TaskStatus::Retired => "This work item has been replaced.",
                TaskStatus::ToDo => "This work item is waiting.",
            }
            .into(),
        };
        Err(format!("Cannot start {}: {reason}", task.name))
    }
}
