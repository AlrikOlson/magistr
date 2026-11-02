use super::*;

impl Edit<'_> {
    pub fn upsert_task(&mut self, task: Task) -> Result<bool, String> {
        // The same refusal as `add_task_with_content`, at the cloud door: a
        // record with no id cannot be addressed by anything downstream. The
        // push lane already skips these (`cloud::backfill`), so accepting one
        // here would only re-import a defect the other doors now reject.
        if task.id.trim().is_empty() {
            return Err("a task needs an id".into());
        }
        if task.project_id.as_deref() != Some(self.project_id.as_str()) {
            return Err("The work item belongs to another project.".into());
        }
        match self.index_of(&task.id) {
            Ok(idx) => {
                if !crate::plan::domain::task_wins(&task, &self.plan.tasks[idx]) {
                    return Ok(false); // The stored copy is at least as new.
                }
                self.plan.tasks[idx] = task;
            }
            Err(_) => self.plan.tasks.push(task),
        }
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)] // Mirrors the public task creation command.
    pub fn add_task(
        &mut self,
        id: String,
        title: String,
        status: TaskStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        shared: bool,
    ) -> Result<&Task, String> {
        self.add_task_with_content(
            id,
            title,
            String::new(),
            status,
            priority,
            description,
            acceptance,
            deps,
            shared,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_task_with_content(
        &mut self,
        id: String,
        title: String,
        name: String,
        status: TaskStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        shared: bool,
        content: Option<crate::content::StructuredContent>,
        feature: Option<String>,
    ) -> Result<&Task, String> {
        // The id is the one field every other verb keys on: a task without
        // one cannot be linked, mutated, mirrored, or drawn. Two real stores
        // held such a record — both admitted here, back when this door only
        // checked for duplicates. This check is what makes `name::derive`'s
        // "the engine rejects that id upstream" true.
        if id.trim().is_empty() {
            return Err(
                "a task needs an id — it is the field every other verb addresses it by".to_string(),
            );
        }
        if self.plan.tasks.iter().any(|c| c.id == id) {
            return Err(format!("task '{id}' already exists"));
        }
        // "Required or derived, never absent": a caller may omit the name and
        // get a usable one, but no path exists that creates a nameless task.
        // An over-budget name IS rejected — the whole point of the field is that
        // a sentence cannot be what a node wears, and accepting one here would
        // be the leak that reopens the defect.
        let name = match name.trim() {
            "" => crate::plan::name::derive(&id),
            given => {
                if let Some(why) = crate::plan::name::why_unfit(given) {
                    return Err(format!("task '{id}': {why}"));
                }
                given.to_string()
            }
        };
        // The feature is validated BEFORE anything is written, and rejected
        // as a whole call rather than dropped. A task born unassigned because
        // its feature was quietly ignored is invisible to every focused read —
        // `next_in_feature` will not offer it and the frontier will not count
        // it — and nothing about the created task says why. Refusing here is
        // the only outcome a caller can act on.
        let feature = Self::clean_group(feature);
        if let Some(name) = &feature
            && let Some(why) = self.why_feature_unfit(name, Some(&id))
        {
            return Err(format!("cannot feature task '{id}': {why}"));
        }
        if let Some(content) = &content {
            content.validate()?;
        }
        let now = Self::now();
        // Captured before `id` moves into the task: the inherited opt-in below
        // needs to name it, and the status decides whether it qualifies.
        let born = id.clone();
        let born_active = status.is_active();
        self.plan.tasks.push(Task {
            tier: None,
            id,
            title,
            name,
            status,
            priority,
            description,
            content,
            notes: String::new(),
            feature,
            acceptance,
            deps,
            refs: Vec::new(),
            shared,
            reprioritize: None,
            status_proposal: None,
            title_proposal: None,
            retired_reason: None,
            waiting_on: None,
            // Stamped at birth: this engine is this project, so a task it
            // records is provably ours and every later copy carries the proof.
            project_id: Some(self.project_id.clone()),
            created_at: now.clone(),
            updated_at: now,
        });
        let snapshot = self.plan.tasks.last().unwrap().clone();
        self.record_event(PlanFrame::TaskAdded { task: snapshot });

        // THE INHERITANCE, and the only place it happens
        // (tracker-optin-never-grows). A project whose owner explicitly ran
        // `tracker on` should not have to re-consent to every task the plan
        // grows; the argument for reversing the silent-by-default rule, and the
        // reason it costs nothing, is written at
        // [`crate::tracker::config::inherited_opt_in`], which is what decides
        // whether this field is set at all.
        //
        // That this is inside `add_task` — reachable only by a task coming
        // into existence — is what makes "never a retroactive sweep" structural
        // rather than a promise: there is no loop here that could reach a task
        // that already exists, so there is nothing to accidentally point at one.
        if born_active && let Some(provider) = self.inherit_opt_in.clone() {
            self.set_tracker_opt_in(&born, &provider, true)?;
        }

        self.plan
            .tasks
            .iter()
            .find(|c| c.id == born)
            .ok_or_else(|| "task vanished after add".to_string())
    }

    pub fn set_status(&mut self, id: &str, to: TaskStatus) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        let from = self.plan.tasks[idx].status;
        // Starting scheduled work requires readiness. Done -> InProgress is
        // completion revocation for an existing session, not a new start.
        if to == TaskStatus::InProgress && matches!(from, TaskStatus::ToDo | TaskStatus::Waiting) {
            self.require_ready(&self.plan.tasks[idx])?;
        }
        if !from.allows(to) {
            return Err(format!(
                "illegal transition for task '{id}': {from:?} -> {to:?}"
            ));
        }
        let c = &mut self.plan.tasks[idx];
        c.status = to;
        c.status_proposal = None;
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_task(
        &mut self,
        id: &str,
        title: Option<String>,
        priority: Option<u32>,
        description: Option<String>,
        acceptance: Option<Vec<String>>,
        deps: Option<Vec<String>>,
        content: Option<crate::content::StructuredContent>,
    ) -> Result<&Task, String> {
        if let Some(content) = &content {
            content.validate()?;
        }
        let idx = self.index_of(id)?;
        let c = &mut self.plan.tasks[idx];
        if let Some(t) = title {
            // An explicit title edit is the human speaking about exactly the
            // field a title proposal is about, so it disposes the proposal —
            // otherwise the projector would keep sending the conceded value
            // and the edit the human just made could never reach the tracker.
            if c.title != t {
                c.title_proposal = None;
            }
            c.title = t;
        }
        if let Some(p) = priority {
            // Same rule as the title edit above, for the reprioritize sibling:
            // an explicit priority edit is the human acting on exactly the
            // field the proposal is about, so it disposes the proposal.
            if c.priority != p {
                c.reprioritize = None;
            }
            c.priority = p;
        }
        if let Some(d) = description {
            c.description = d;
        }
        if let Some(a) = acceptance {
            c.acceptance = a;
        }
        if let Some(d) = deps {
            c.deps = d;
        }
        if let Some(sc) = content {
            c.content = Some(sc);
        }
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn set_feature(&mut self, id: &str, feature: Option<String>) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        let cleaned = Self::clean_group(feature);
        if let Some(name) = &cleaned
            && let Some(why) = self.why_feature_unfit(name, None)
        {
            return Err(format!("cannot feature task '{id}': {why}"));
        }
        let c = &mut self.plan.tasks[idx];
        if c.feature == cleaned {
            // No-op writes still cost a persist and an event, and a projector
            // that re-groups every push would churn the tracker for nothing.
            return Ok(&self.plan.tasks[idx]);
        }
        c.feature = cleaned;
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn set_name(&mut self, id: &str, name: &str) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        let cleaned = match name.trim() {
            "" => crate::plan::name::derive(id),
            given => {
                if let Some(why) = crate::plan::name::why_unfit(given) {
                    return Err(format!("task '{id}': {why}"));
                }
                given.to_string()
            }
        };
        let c = &mut self.plan.tasks[idx];
        if c.name == cleaned {
            // No-op writes still cost a persist and an event, and the projector
            // would churn the tracker for nothing — same reasoning as set_feature.
            return Ok(&self.plan.tasks[idx]);
        }
        c.name = cleaned;
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn focus_set(
        &mut self,
        team: &str,
        feature: &str,
        mode: FocusMode,
    ) -> Result<Focus, String> {
        let team = validate_team(team)?;
        let resolved = match self.features().resolve_current(feature) {
            FeatureResolution::Current { feature } => feature,
            other => return Err(other.explain(feature)),
        };
        let now = Self::now();
        let focus = match self.plan.focuses.iter_mut().find(|f| f.team == team) {
            Some(existing) => {
                existing.feature = resolved;
                existing.mode = mode;
                existing.updated_at = now;
                existing.clone()
            }
            None => {
                let focus = Focus {
                    team,
                    feature: resolved,
                    mode,
                    set_at: now.clone(),
                    updated_at: now,
                };
                self.plan.focuses.push(focus.clone());
                focus
            }
        };
        Ok(focus)
    }

    pub fn focus_clear(&mut self, team: &str) -> Result<bool, String> {
        let team = validate_team(team)?;
        let before = self.plan.focuses.len();
        self.plan.focuses.retain(|f| f.team != team);
        let removed = self.plan.focuses.len() != before;
        Ok(removed)
    }

    pub fn retire_task(&mut self, id: &str, reason: String) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        let from = self.plan.tasks[idx].status;
        if !from.allows(TaskStatus::Retired) {
            return Err(format!("cannot obsolete task '{id}' from status {from:?}"));
        }
        let c = &mut self.plan.tasks[idx];
        c.status = TaskStatus::Retired;
        // The one status write outside `set_status`, so the field-scoped
        // disposal rule applies here too: obsoleting is the strongest possible
        // human statement about the status field.
        c.status_proposal = None;
        c.retired_reason = Some(reason);
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskRetired { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn start_task(&mut self, id: &str) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        self.require_ready(&self.plan.tasks[idx])?;
        self.set_status(id, TaskStatus::InProgress)
    }

    pub fn complete_task(&mut self, id: &str, step_ref: Option<&str>) -> Result<&Task, String> {
        if let Some(raw) = step_ref {
            // Validate before transitioning so a bad ref doesn't half-apply.
            self.normalize_ref(raw)?;
        }
        self.set_status(id, TaskStatus::Done)?;
        if let Some(raw) = step_ref {
            self.push_cross_ref(id, raw)?;
        }
        let idx = self.index_of(id)?;
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskCompleted { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn link_task(&mut self, id: &str, cross_ref: &str) -> Result<&Task, String> {
        self.push_cross_ref(id, cross_ref)?;
        let idx = self.index_of(id)?;
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn set_waiting_on(&mut self, id: &str, waiting_on: WaitingOn) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        let c = &mut self.plan.tasks[idx];
        c.waiting_on = Some(waiting_on);
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }

    pub fn clear_waiting_on(&mut self, id: &str) -> Result<&Task, String> {
        let idx = self.index_of(id)?;
        if self.plan.tasks[idx].waiting_on.take().is_none() {
            return Err(format!("task '{id}' has no waiting reason to clear"));
        }
        let c = &mut self.plan.tasks[idx];
        c.updated_at = Self::now();
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });
        Ok(&self.plan.tasks[idx])
    }
}
