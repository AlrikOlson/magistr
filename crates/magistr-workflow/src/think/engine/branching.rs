//! Branch creation — opening a fork, picking an id, depth bookkeeping.
//!
//! Branch *lifecycle* (active → merged → abandoned) lives in
//! [`super::mutations::ReasoningServer::set_branch_status`](super::ReasoningServer::set_branch_status); this file
//! owns the "create or extend a branch" side. The
//! `branch_depth_cache` makes repeated depth lookups O(1) once a step
//! is on a branch — invalidated only when a new branch is registered.

use std::collections::HashMap;

use chrono::Utc;

use crate::think::domain::{Branch, BranchStatus, ThinkHistory, ThinkStep};

use super::core::ReasoningServer;

impl ReasoningServer {
    /// Restore recorded lifecycle metadata first, then fill legacy histories
    /// from their members. Stored status and merge destination are authoritative.
    pub(crate) fn restored_branches(history: &ThinkHistory) -> HashMap<String, Branch> {
        let mut branches: HashMap<String, Branch> = history
            .branches
            .iter()
            .flatten()
            .map(|branch| {
                let mut restored = branch.clone();
                // Canonical history carries renumbered/revised records. Stored
                // member copies may predate startup repair.
                restored.steps.clear();
                (restored.id.clone(), restored)
            })
            .collect();
        for step in &history.steps {
            let Some(id) = &step.branch_id else {
                continue;
            };
            if !branches.contains_key(id) {
                let Some(from) = step.branch_from else {
                    continue;
                };
                let depth = history
                    .steps
                    .iter()
                    .find(|s| s.step_number == from)
                    .and_then(|s| s.branch_id.as_ref())
                    .and_then(|parent| branches.get(parent))
                    .map_or(1, |parent| parent.depth + 1);
                branches.insert(
                    id.clone(),
                    Branch {
                        id: id.clone(),
                        name: step
                            .branch_name
                            .clone()
                            .unwrap_or_else(|| "Unnamed question".into()),
                        from_step: from,
                        steps: Vec::new(),
                        status: BranchStatus::Active,
                        created_at: step.timestamp.clone().unwrap_or_default(),
                        depth,
                        merged_into: None,
                    },
                );
            }
            if let Some(branch) = branches.get_mut(id) {
                if let Some(member) = branch
                    .steps
                    .iter_mut()
                    .find(|s| s.step_number == step.step_number)
                {
                    *member = step.clone();
                } else {
                    branch.steps.push(step.clone());
                }
            }
        }
        branches
    }

    pub fn calculate_branch_depth(&mut self, step_number: u32) -> u32 {
        if let Some(&v) = self.branch_depth_cache.get(&step_number) {
            return v;
        }
        let depth = if let Some(branch_id) = self.step_to_branch.get(&step_number) {
            self.branches
                .get(branch_id)
                .map(|b| b.depth + 1)
                .unwrap_or(1)
        } else {
            1
        };
        self.branch_depth_cache.insert(step_number, depth);
        depth
    }

    pub(crate) fn invalidate_branch_depth_cache(&mut self) {
        self.branch_depth_cache.clear();
    }

    pub(crate) fn next_branch_id(&mut self) -> String {
        self.branches_seq += 1;
        format!("branch-{}-{:04}", Self::now_ms(), self.branches_seq)
    }

    /// Validate branch metadata without changing stored records. Continuing an
    /// existing branch needs only its id; every member retains the same origin
    /// and question so broadcasts and persisted steps describe the same path.
    pub(crate) fn resolve_branch(&self, step: &mut ThinkStep) -> Result<(), String> {
        if step.branch_from.is_none() && step.branch_id.is_none() {
            if step.branch_name.is_some() {
                return Err("A new branch needs branch_from and a question in branch_name".into());
            }
            return Ok(());
        }
        if !self.config.features.enable_branching {
            return Err("Branching is disabled in the project configuration".into());
        }
        if let Some(id) = step.branch_id.as_deref() {
            if id.trim().is_empty() {
                return Err("branch_id must not be blank".into());
            }
            if let Some(branch) = self.branches.get(id) {
                if step
                    .branch_from
                    .is_some_and(|from| from != branch.from_step)
                {
                    return Err(format!(
                        "Branch {id} began at step {}; keep that branch_from or omit it",
                        branch.from_step
                    ));
                }
                if step
                    .branch_name
                    .as_ref()
                    .is_some_and(|name| name != &branch.name)
                {
                    return Err(format!(
                        "Branch {id} is named {:?}; keep that branch_name or omit it",
                        branch.name
                    ));
                }
                step.branch_from = Some(branch.from_step);
                step.branch_name = Some(branch.name.clone());
                return Ok(());
            }
            if step.branch_from.is_none() {
                return Err(format!(
                    "Unknown branch_id {id:?}; use a returned branch id, or branch_from to start a new branch"
                ));
            }
        }
        let Some(from) = step.branch_from else {
            return Ok(());
        };
        if step
            .branch_name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err("branch_name must not be blank".into());
        }
        if from == step.step_number {
            let msg = format!("Cannot branch from self (step {})", step.step_number);
            tracing::warn!("{msg}");
            return Err(msg);
        }
        if !self.step_numbers.contains(&from) {
            let available = self.sorted_step_numbers();
            let msg = format!(
                "Cannot branch from step {from}: step does not exist. Available steps: {}",
                if available.is_empty() {
                    "none".into()
                } else {
                    available
                }
            );
            tracing::warn!("{msg}");
            return Err(msg);
        }

        let depth = self
            .step_to_branch
            .get(&from)
            .and_then(|id| self.branches.get(id))
            .map_or(1, |branch| branch.depth + 1);
        if depth > self.config.system.max_branch_depth {
            return Err(format!(
                "Branch depth {depth} exceeds maximum {}",
                self.config.system.max_branch_depth
            ));
        }
        Ok(())
    }

    /// Register a branch that has already passed resolve_branch.
    pub(crate) fn handle_branching(&mut self, step: &mut ThinkStep) {
        let Some(from) = step.branch_from else {
            return;
        };
        let branch_id = step
            .branch_id
            .clone()
            .unwrap_or_else(|| self.next_branch_id());
        let branch_name = step
            .branch_name
            .clone()
            .unwrap_or_else(|| format!("Alternative {}", self.branches.len() + 1));

        if !self.branches.contains_key(&branch_id) {
            let depth = self.calculate_branch_depth(from);
            let branch = Branch {
                id: branch_id.clone(),
                name: branch_name.clone(),
                from_step: from,
                steps: Vec::new(),
                status: BranchStatus::Active,
                created_at: Utc::now().to_rfc3339(),
                depth,
                merged_into: None,
            };
            self.branches.insert(branch_id.clone(), branch);
            self.invalidate_branch_depth_cache();
            if let Some(meta) = self.history.metadata.as_mut() {
                meta.branches_created = Some(meta.branches_created.unwrap_or(0) + 1);
            }
            eprintln!("🌿 Created branch \"{branch_name}\" from step {from} (depth: {depth})");
        }

        step.branch_id = Some(branch_id.clone());
        step.branch_name = Some(branch_name);
        if let Some(branch) = self.branches.get_mut(&branch_id) {
            branch.steps.push(step.clone());
        }
        self.step_to_branch.insert(step.step_number, branch_id);
    }
}
