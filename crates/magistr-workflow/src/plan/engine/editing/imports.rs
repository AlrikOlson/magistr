use super::*;

impl Edit<'_> {
    pub fn record_refresh(
        &mut self,
        summary: String,
        think_steps: Vec<u32>,
    ) -> Result<usize, String> {
        self.plan.refreshes.push(RefreshNote {
            at: Self::now(),
            summary: summary.clone(),
            think_steps: think_steps.clone(),
        });
        self.record_event(PlanFrame::RefreshRecorded {
            summary,
            think_steps,
        });
        Ok(self.plan.refreshes.len())
    }

    pub fn merge_from_import(
        &mut self,
        imported: crate::plan::import::ImportedPlan,
    ) -> Result<(usize, usize), String> {
        if self.plan.preamble.is_empty() && !imported.preamble.is_empty() {
            self.plan.preamble = imported.preamble;
        }
        let existing_notes: std::collections::HashSet<String> = self
            .plan
            .notes
            .iter()
            .map(|n| n.heading.to_ascii_lowercase())
            .collect();
        for n in imported.notes {
            if !existing_notes.contains(&n.heading.to_ascii_lowercase()) {
                self.plan.notes.push(n);
            }
        }
        let now = Self::now();
        let (mut added, mut updated) = (0, 0);
        for c in imported.tasks {
            if let Some(existing) = self.plan.tasks.iter_mut().find(|x| x.id == c.id) {
                let mut changed = false;
                if existing.notes.is_empty() && !c.notes.is_empty() {
                    existing.notes = c.notes;
                    changed = true;
                }
                if existing.description.is_empty() && !c.description.is_empty() {
                    existing.description = c.description;
                    changed = true;
                }
                for a in c.acceptance {
                    if !existing.acceptance.contains(&a) {
                        existing.acceptance.push(a);
                        changed = true;
                    }
                }
                if changed {
                    existing.updated_at = now.clone();
                    updated += 1;
                }
            } else {
                let name = c.resolved_name();
                self.plan.tasks.push(Task {
                    tier: None,
                    id: c.id,
                    title: c.title,
                    name,
                    status: c.status,
                    priority: c.priority,
                    description: c.description,
                    content: None,
                    notes: c.notes,
                    feature: None,
                    acceptance: c.acceptance,
                    deps: c.deps,
                    refs: Vec::new(),
                    shared: false,
                    reprioritize: None,
                    status_proposal: None,
                    title_proposal: None,
                    retired_reason: None,
                    waiting_on: None,
                    project_id: Some(self.project_id.clone()),
                    created_at: now.clone(),
                    updated_at: now.clone(),
                });
                added += 1;
            }
        }
        Ok((added, updated))
    }

    pub fn seed_from_import(
        &mut self,
        imported: crate::plan::import::ImportedPlan,
    ) -> Result<usize, String> {
        if self.plan.preamble.is_empty() && !imported.preamble.is_empty() {
            self.plan.preamble = imported.preamble;
        }
        let existing_notes: std::collections::HashSet<String> = self
            .plan
            .notes
            .iter()
            .map(|n| n.heading.to_ascii_lowercase())
            .collect();
        for n in imported.notes {
            if !existing_notes.contains(&n.heading.to_ascii_lowercase()) {
                self.plan.notes.push(n);
            }
        }
        let now = Self::now();
        let mut added = 0;
        for c in imported.tasks {
            if self.plan.tasks.iter().any(|x| x.id == c.id) {
                continue;
            }
            let name = c.resolved_name();
            self.plan.tasks.push(Task {
                tier: None,
                id: c.id,
                title: c.title,
                name,
                status: c.status,
                priority: c.priority,
                description: c.description,
                content: None,
                notes: c.notes,
                feature: None,
                acceptance: c.acceptance,
                deps: c.deps,
                refs: Vec::new(),
                shared: false,
                reprioritize: None,
                status_proposal: None,
                title_proposal: None,
                retired_reason: None,
                waiting_on: None,
                project_id: Some(self.project_id.clone()),
                created_at: now.clone(),
                updated_at: now.clone(),
            });
            added += 1;
        }
        Ok(added)
    }
}
