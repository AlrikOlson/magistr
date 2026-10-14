//! Import authored identities without treating numbers or prose as identity.
use std::collections::{HashMap, HashSet};

use chrono::Utc;
use magistr_workflow_contracts::Observed;

use super::ReasoningServer;
use crate::think::domain::{DepEdge, StepError, ThinkStep};

impl ReasoningServer {
    /// Import a coherent batch silently; return a count only after committing it.
    pub fn adopt_steps(&mut self, steps: Vec<ThinkStep>) -> Result<Observed<usize>, StepError> {
        let persistence = self.persistence.clone();
        let project_id = self.project_id.clone();
        self.transact_project(self.active_session.clone(), move |project| {
            let known: HashMap<String, ThinkStep> = project.histories.values()
                .flat_map(|history| &history.steps)
                .map(|step| (step.record_id.clone().expect("validated identity"), step.clone()))
                .collect();
            let mut used: HashSet<u32> = known.values().map(|step| step.step_number).collect();
            let mut highest = used.iter().copied().max().unwrap_or(0);
            let mut by_number: HashMap<u32, HashSet<String>> = HashMap::new();
            let mut unique: HashMap<String, ThinkStep> = HashMap::new();
            let mut order = Vec::new();
            for step in steps {
                let id = step.record_id.as_deref().filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| conflict("An imported reasoning item has no record identity."))?
                    .to_owned();
                if step.step_number == 0 {
                    return Err(conflict("An imported reasoning item has no number."));
                }
                let session = step.session_id.as_deref().unwrap_or(&project_id);
                persistence.validate_session_id(session)
                    .map_err(|error| conflict(&error.to_string()))?;
                by_number.entry(step.step_number).or_default().insert(id.clone());
                if let Some(prior) = unique.get(&id) {
                    if prior != &step {
                        return Err(conflict("The import contains conflicting copies of one reasoning item."));
                    }
                    continue;
                }
                order.push(id.clone());
                unique.insert(id, step);
            }
            let mut assigned = HashMap::new();
            for id in &order {
                let step = &unique[id];
                let number = if let Some(existing) = known.get(id) {
                    existing.step_number
                } else if used.insert(step.step_number) {
                    highest = highest.max(step.step_number);
                    step.step_number
                } else {
                    highest = highest.checked_add(1)
                        .ok_or_else(|| conflict("There are no unused reasoning numbers left."))?;
                    used.insert(highest);
                    highest
                };
                assigned.insert(id.clone(), number);
            }
            let remap = |number: u32| -> Result<u32, StepError> {
                match by_number.get(&number) {
                    Some(ids) if ids.len() == 1 => Ok(assigned[ids.iter().next().expect("one id")]),
                    Some(_) => Err(conflict("An imported reference matches several distinct reasoning items. Include an unambiguous history before retrying.")),
                    None if known.values().any(|step| step.step_number == number) => Ok(number),
                    None => Err(conflict("An imported reference has no corresponding reasoning item. Include the referenced history before retrying.")),
                }
            };
            let mut adopted = 0;
            let mut changed_sessions = HashSet::new();
            for id in order {
                let mut step = unique.remove(&id).expect("ordered identity");
                step.step_number = assigned[&id];
                step.revises_step = step.revises_step.map(&remap).transpose()?;
                step.revised_by = step.revised_by.map(&remap).transpose()?;
                step.branch_from = step.branch_from.map(&remap).transpose()?;
                if let Some(dependencies) = &mut step.dependencies {
                    for dependency in dependencies {
                        match dependency {
                            DepEdge::Bare(number) | DepEdge::Tagged { step: number, .. } => {
                                *number = remap(*number)?;
                            }
                        }
                    }
                }
                if let Some(existing) = known.get(&id) {
                    if !same_authored_record(existing, &step, &project_id) {
                        return Err(conflict("An imported reasoning identity has different authored content. Resolve the conflicting record before retrying."));
                    }
                    continue;
                }
                let session = step.session_id.clone().unwrap_or_else(|| project_id.clone());
                let history = project.histories.entry(session.clone()).or_insert_with(|| {
                    let mut history = Self::new_history();
                    history.session_id = (session != project_id).then(|| session.clone());
                    history.metadata.as_mut().expect("new history metadata").project_id = Some(project_id.clone());
                    history
                });
                if let Some(branch_id) = &step.branch_id {
                    if let Some(branch) = history.branches.iter().flatten().find(|branch| &branch.id == branch_id) {
                        if step.branch_from != Some(branch.from_step)
                            || step.branch_name.as_deref() != Some(branch.name.as_str())
                        {
                            return Err(conflict("An imported question disagrees with its saved origin or name."));
                        }
                    } else if step.branch_from.is_none() || step.branch_name.as_deref().is_none_or(|name| name.trim().is_empty()) {
                        return Err(conflict("An imported question is missing its origin or name."));
                    }
                }
                history.completed |= step.is_final_step == Some(true);
                history.steps.push(step);
                history.updated_at = Some(Utc::now().to_rfc3339());
                changed_sessions.insert(session);
                adopted += 1;
            }
            for session in changed_sessions {
                let history = project.histories.get_mut(&session).expect("changed history");
                history.steps.sort_by_key(|step| step.step_number);
                let mut branches: Vec<_> = Self::restored_branches(history).into_values().collect();
                branches.sort_by(|a, b| a.from_step.cmp(&b.from_step).then(a.id.cmp(&b.id)));
                history.branches = Some(branches);
                let meta = history.metadata.get_or_insert_with(Default::default);
                meta.project_id = Some(project_id.clone());
                let mut tools: Vec<_> = history.steps.iter().flat_map(Self::extract_tools_used).collect();
                tools.sort();
                tools.dedup();
                meta.tools_used = Some(tools);
            }
            Ok(adopted)
        })
    }
}

fn conflict(message: &str) -> StepError {
    StepError::rejected(
        message,
        "Resolve the conflicting reasoning identities or references, then retry.",
    )
}

fn same_authored_record(left: &ThinkStep, right: &ThinkStep, project_id: &str) -> bool {
    fn authored(mut step: ThinkStep, project_id: &str) -> ThinkStep {
        // These fields have explicit local mutators or are projections of identity.
        step.step_number = 0;
        step.estimated_total = 0;
        step.pinned = None;
        step.revised_by = None;
        if step.session_id.as_deref() == Some(project_id) {
            step.session_id = None;
        }
        step
    }
    authored(left.clone(), project_id) == authored(right.clone(), project_id)
}
