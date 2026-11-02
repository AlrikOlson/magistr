use super::*;

impl Edit<'_> {
    pub fn propose_status(
        &mut self,
        id: &str,
        suggested_status: TaskStatus,
        reason: String,
        source: String,
    ) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        let unchanged = self.plan.tasks[idx]
            .status_proposal
            .as_ref()
            .is_some_and(|p| {
                p.suggested_status == suggested_status && p.reason == reason && p.source == source
            });
        if unchanged {
            return Ok(&self.plan.tasks[idx]);
        }

        let c = &mut self.plan.tasks[idx];
        c.status_proposal = Some(crate::plan::domain::StatusProposal {
            suggested_status,
            reason,
            proposed_at: Self::now(),
            source,
        });
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn propose_title(
        &mut self,
        id: &str,
        suggested_title: String,
        reason: String,
        source: String,
    ) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        let unchanged = self.plan.tasks[idx]
            .title_proposal
            .as_ref()
            .is_some_and(|p| {
                p.suggested_title == suggested_title && p.reason == reason && p.source == source
            });
        if unchanged {
            return Ok(&self.plan.tasks[idx]);
        }

        let c = &mut self.plan.tasks[idx];
        c.title_proposal = Some(crate::plan::domain::TitleProposal {
            suggested_title,
            reason,
            proposed_at: Self::now(),
            source,
        });
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn resolve_title_proposal(&mut self, id: &str, accept: bool) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        let Some(proposal) = self.plan.tasks[idx].title_proposal.take() else {
            return Err(format!("task '{id}' has no open title proposal"));
        };
        let c = &mut self.plan.tasks[idx];
        if accept {
            c.title = proposal.suggested_title;
        }
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn propose_reprioritize(
        &mut self,
        id: &str,
        suggested_priority: u32,
        reason: String,
    ) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        let unchanged = self.plan.tasks[idx]
            .reprioritize
            .as_ref()
            .is_some_and(|p| p.suggested_priority == suggested_priority && p.reason == reason);
        if unchanged {
            return Ok(&self.plan.tasks[idx]);
        }

        let c = &mut self.plan.tasks[idx];
        c.reprioritize = Some(ReprioritizeProposal {
            suggested_priority,
            reason,
            proposed_at: Self::now(),
        });
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn resolve_reprioritize(&mut self, id: &str, accept: bool) -> Result<&Task, String> {
        let index = self.index_of(id)?;
        let Some(proposal) = self.plan.tasks[index].reprioritize.take() else {
            return Err(format!("task '{id}' has no open reprioritize proposal"));
        };
        let task = &mut self.plan.tasks[index];
        if accept {
            task.priority = proposal.suggested_priority;
        }
        task.updated_at = Self::now();
        let task = self.plan.tasks[index].clone();
        self.record_event(PlanFrame::TaskChanged { task });
        Ok(&self.plan.tasks[index])
    }

    pub fn resolve_status_proposal(&mut self, id: &str, accept: bool) -> Result<&Task, String> {
        let index = self.index_of(id)?;
        let task = &self.plan.tasks[index];
        let Some(proposal) = &task.status_proposal else {
            return Err(format!("task '{id}' has no open status proposal"));
        };
        let status = if accept {
            proposal.suggested_status
        } else {
            task.status
        };
        self.set_status(id, status)
    }
}
