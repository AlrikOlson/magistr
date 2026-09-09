use std::collections::BTreeSet;

use crate::error::EngineError;
use crate::fleet::{JobState, WorkQueue, WorkUnit};

use super::batch_row::BatchRow;
use super::batch_state::BatchState;
use super::batch_status::BatchStatus;
use super::project_graph::ProjectGraph;

/// One dependency-ordered run of a prompt across projects.
///
/// The batch records which projects were released and which were skipped;
/// all job states come from [`WorkQueue`]. Job IDs use `<batch>:<project>`
/// so callers can identify a project's job before submission.
///
/// [`plan`] rejects cycles. [`advance`] releases a project only after its
/// dependencies succeed, skips dependents of failures, and keeps independent
/// branches running. Projects sharing a [`ProjectDep::worktree`] cannot be
/// in flight together, protecting the shared Git index from concurrent writes.
/// [`status`] rolls these derived states into a single [`BatchStatus`].
///
/// [`ProjectDep::worktree`]: super::project_dep::ProjectDep::worktree
/// [`plan`]: BatchRun::plan
/// [`advance`]: BatchRun::advance
/// [`status`]: BatchRun::status
#[derive(Debug, Clone)]
pub struct BatchRun {
    id: String,
    prompt: String,
    requested_by: String,
    graph: ProjectGraph,
    order: Vec<String>,
    released: BTreeSet<String>,
    skipped: BTreeSet<String>,
}

impl BatchRun {
    /// Plan a run of `prompt` across `graph`, refusing a graph no order
    /// satisfies.
    pub fn plan(
        id: impl Into<String>,
        prompt: impl Into<String>,
        graph: ProjectGraph,
    ) -> Result<Self, EngineError> {
        let order = graph.order()?;
        Ok(Self {
            id: id.into(),
            prompt: prompt.into(),
            requested_by: String::new(),
            graph,
            order,
            released: BTreeSet::new(),
            skipped: BTreeSet::new(),
        })
    }

    /// Name the client this run's streams belong to (self-consuming
    /// builder). Carried onto every [`WorkUnit`] the run submits, so a
    /// batch's jobs are attributed the way a single submission is.
    #[must_use]
    pub fn requested_by(mut self, client: impl Into<String>) -> Self {
        self.requested_by = client.into();
        self
    }

    /// The batch id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The prompt every project in the run is given.
    #[must_use]
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// The projects, in the order their dependencies allow.
    #[must_use]
    pub fn order(&self) -> &[String] {
        &self.order
    }

    /// The projects and the edges between them.
    #[must_use]
    pub fn graph(&self) -> &ProjectGraph {
        &self.graph
    }

    /// The job id a project runs under — derived, never stored.
    #[must_use]
    pub fn job_id(&self, project: &str) -> String {
        format!("{}:{project}", self.id)
    }

    // ── Derived state ───────────────────────────────────────────────

    /// Where `project` stands, read off the queue wherever the queue can
    /// answer.
    ///
    /// A released project whose job the queue no longer holds reads as
    /// [`BatchState::Queued`]: the batch knows it asked for the work and
    /// will not ask twice, and "waiting to be picked up" is the honest
    /// reading of a job an operator retired out from under a live run.
    #[must_use]
    pub fn state_of(&self, queue: &WorkQueue, project: &str) -> BatchState {
        if self.skipped.contains(project) {
            return BatchState::Skipped;
        }
        if !self.released.contains(project) {
            return BatchState::Waiting;
        }
        match queue.job(&self.job_id(project)).map(|job| job.state) {
            Some(JobState::Claimed) => BatchState::Running,
            Some(JobState::Completed) => BatchState::Succeeded,
            Some(JobState::Failed) => BatchState::Failed,
            Some(JobState::Queued) | None => BatchState::Queued,
        }
    }

    /// The whole run in one answer, in dependency order.
    #[must_use]
    pub fn status(&self, queue: &WorkQueue) -> BatchStatus {
        let rows = self
            .order
            .iter()
            .map(|project| BatchRow {
                project: project.clone(),
                state: self.state_of(queue, project),
                job_id: self.job_id(project),
            })
            .collect();
        BatchStatus::from_rows(&self.id, rows)
    }

    /// Whether every project has settled.
    #[must_use]
    pub fn is_settled(&self, queue: &WorkQueue) -> bool {
        self.order
            .iter()
            .all(|project| self.state_of(queue, project).is_terminal())
    }

    // ── Advancing ───────────────────────────────────────────────────

    /// Fold what the queue now says into the run: cancel what a failure
    /// took down, then release everything whose turn has come.
    ///
    /// Returns the projects released by this call, in dependency order.
    /// Idempotent in the sense that matters — calling it twice with
    /// nothing changed underneath releases nothing the second time —
    /// which is what lets `batch.status` sweep before it answers the way
    /// `work.status` does, and the control plane need no background
    /// timer to make progress.
    ///
    /// It consults no clock, which is deliberate: the batch schedules
    /// *projects* and the queue schedules *machines*, so nothing here
    /// needs the fleet or the time — the two halves meet only where a
    /// job is submitted.
    pub fn advance(&mut self, queue: &mut WorkQueue) -> Result<Vec<String>, EngineError> {
        self.cancel_downstream_of_failures(queue);
        self.release_ready(queue)
    }

    /// Mark the transitive dependents of every failed project skipped.
    ///
    /// A released project is never touched: it can only have been
    /// released once all its dependencies succeeded, so it is not
    /// downstream of anything that has failed, and skipping something
    /// already running would be a lie about work a node is doing.
    fn cancel_downstream_of_failures(&mut self, queue: &WorkQueue) {
        let failed: Vec<String> = self
            .order
            .iter()
            .filter(|project| self.state_of(queue, project) == BatchState::Failed)
            .cloned()
            .collect();

        for project in failed {
            for downstream in self.graph.dependents_of(&project) {
                if !self.released.contains(&downstream) {
                    self.skipped.insert(downstream);
                }
            }
        }
    }

    /// Submit a job for every project whose dependencies have all
    /// succeeded and whose working tree is free.
    fn release_ready(&mut self, queue: &mut WorkQueue) -> Result<Vec<String>, EngineError> {
        let mut occupied = self.busy_worktrees(queue);
        let mut released = Vec::new();

        for project in self.order.clone() {
            if self.released.contains(&project) || self.skipped.contains(&project) {
                continue;
            }
            if !self.deps_all_succeeded(queue, &project) {
                continue;
            }
            let worktree = self
                .graph
                .get(&project)
                .and_then(|dep| dep.worktree.clone());
            if let Some(tree) = &worktree
                && occupied.contains(tree)
            {
                continue;
            }

            let unit = WorkUnit::new(self.job_id(&project), &project, &self.prompt)
                .requested_by(&self.requested_by);
            queue.submit(unit)?;

            self.released.insert(project.clone());
            if let Some(tree) = worktree {
                occupied.insert(tree);
            }
            released.push(project);
        }
        Ok(released)
    }

    /// Whether every in-batch dependency of `project` has succeeded.
    ///
    /// The bar is *succeeded* rather than *settled* on purpose: a
    /// dependency that failed or was skipped leaves its dependents where
    /// [`cancel_downstream_of_failures`] can reach them, and never
    /// releases them by accident in the meantime.
    ///
    /// [`cancel_downstream_of_failures`]: BatchRun::cancel_downstream_of_failures
    fn deps_all_succeeded(&self, queue: &WorkQueue, project: &str) -> bool {
        self.graph
            .deps_of(project)
            .into_iter()
            .all(|dep| self.state_of(queue, dep) == BatchState::Succeeded)
    }

    /// The working trees a project is holding right now.
    fn busy_worktrees(&self, queue: &WorkQueue) -> BTreeSet<String> {
        self.order
            .iter()
            .filter(|project| self.state_of(queue, project).is_in_flight())
            .filter_map(|project| self.graph.get(project).and_then(|dep| dep.worktree.clone()))
            .collect()
    }
}

#[cfg(test)]
#[path = "batch_run.tests.rs"]
mod tests;
