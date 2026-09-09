//! In-memory roadmap index with query and mutation API.
//!
//! `RoadmapIndex` wraps a [`Roadmap`] and provides the typed query/mutation
//! interface consumed by the TUI and MCP tools. It replaces all SQLite query
//! functions from the old `db.rs`.

use crate::error::{MagistrError, Result};
use magistr_engine::spec::SpecSummary;

use super::phase::Phase;
use super::phase_status::PhaseStatus;
use super::schema::Roadmap;
use super::stats::RoadmapStats;
use super::task::Task;
use super::task_id::task_id;
use super::task_match::TaskMatch;

/// Where to insert a phase relative to another phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertPosition<'a> {
    /// Insert before the phase with the given ID.
    Before(&'a str),
    /// Insert after the phase with the given ID.
    After(&'a str),
}

/// In-memory roadmap index with query and mutation API.
///
/// All mutations mark the index as dirty. The caller is responsible for
/// persisting changes (via `YamlStore::save()`).
#[derive(Debug, Clone)]
pub struct RoadmapIndex {
    roadmap: Roadmap,
    dirty: bool,
}

impl RoadmapIndex {
    /// Create an index from a validated roadmap.
    ///
    /// Runs a bottom-up sanitize pass to fix any parent/child checked-state
    /// contradictions (e.g. parent checked but children unchecked).
    #[must_use]
    pub fn new(mut roadmap: Roadmap) -> Self {
        // Sanitize: derive parent checked state from children
        for phase in &mut roadmap.phases {
            for task in &mut phase.tasks {
                sanitize_task_tree(task);
            }
            phase.recompute_status();
        }
        Self {
            roadmap,
            dirty: false,
        }
    }

    /// Whether the index has unsaved mutations.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag (called after saving).
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Get the underlying roadmap (immutable).
    #[must_use]
    pub fn roadmap(&self) -> &Roadmap {
        &self.roadmap
    }

    /// Consume the index and return the owned roadmap.
    #[must_use]
    pub fn into_roadmap(self) -> Roadmap {
        self.roadmap
    }

    // ── Task ID resolution ────────────────────────────────────────────────

    /// Resolve a hierarchical task ID string to `(phase_idx, task_path)`.
    ///
    /// Parses `"E0.2.1"` → `(phase_idx_of_E0, vec![2, 1])`.
    /// The first segment is a phase ID, remaining segments are task indices.
    pub fn resolve_task_id(&self, id: &str) -> Result<(usize, Vec<usize>)> {
        let mut segments = id.split('.');
        let phase_id = segments
            .next()
            .ok_or_else(|| MagistrError::Config(format!("empty task ID: '{id}'")))?;

        let (phase_idx, _) = self.phase_by_id(phase_id).ok_or_else(|| {
            MagistrError::Config(format!("unknown phase ID in task ID '{id}': '{phase_id}'"))
        })?;

        let mut task_path = Vec::new();
        for seg in segments {
            let idx: usize = seg.parse().map_err(|_| {
                MagistrError::Config(format!("invalid index in task ID '{id}': '{seg}'"))
            })?;
            task_path.push(idx);
        }

        if task_path.is_empty() {
            return Err(MagistrError::Config(format!(
                "task ID '{id}' has no task index (expected e.g. '{phase_id}.0')"
            )));
        }

        Ok((phase_idx, task_path))
    }

    // ── Queries ─────────────────────────────────────────────────────────────

    /// Title of the roadmap.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.roadmap.title
    }

    /// Compute aggregate stats.
    #[must_use]
    pub fn stats(&self) -> RoadmapStats {
        let total_phases = self.roadmap.phases.len();
        let completed_phases = self
            .roadmap
            .phases
            .iter()
            .filter(|p| p.status == PhaseStatus::Done)
            .count();
        let total_tasks = self.roadmap.total_tasks();
        let checked_tasks = self.roadmap.checked_tasks();
        RoadmapStats {
            total_phases,
            completed_phases,
            total_tasks,
            checked_tasks,
        }
    }

    /// All phases in order.
    #[must_use]
    pub fn phases(&self) -> &[Phase] {
        &self.roadmap.phases
    }

    /// Look up a phase by its ID.
    #[must_use]
    pub fn phase_by_id(&self, id: &str) -> Option<(usize, &Phase)> {
        self.roadmap
            .phases
            .iter()
            .enumerate()
            .find(|(_, p)| p.id == id)
    }

    /// Tasks for a phase by index.
    #[must_use]
    pub fn tasks(&self, phase_idx: usize) -> Option<&[Task]> {
        self.roadmap
            .phases
            .get(phase_idx)
            .map(|p| p.tasks.as_slice())
    }

    /// Get the next N unchecked tasks across all phases.
    #[must_use]
    pub fn next_unchecked(&self, limit: usize) -> Vec<TaskMatch> {
        let mut results = Vec::new();
        for (pi, phase) in self.roadmap.phases.iter().enumerate() {
            collect_unchecked(&mut results, pi, &phase.id, &phase.name, &phase.tasks, &[]);
            if results.len() >= limit {
                results.truncate(limit);
                return results;
            }
        }
        results
    }

    /// Search for tasks matching a text substring (case-insensitive).
    #[must_use]
    pub fn find_task(&self, query: &str) -> Vec<TaskMatch> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        for (pi, phase) in self.roadmap.phases.iter().enumerate() {
            search_tasks(
                &mut results,
                pi,
                &phase.id,
                &phase.name,
                &phase.tasks,
                &[],
                &query_lower,
            );
        }
        results
    }

    /// Build a [`SpecSummary`] directly from the YAML store data.
    ///
    /// This is the canonical path for workflow prompts — avoids the
    /// roundabout markdown parse and guarantees numbers match `stats()`.
    #[must_use]
    pub fn to_spec_summary(&self) -> SpecSummary {
        let stats = self.stats();
        let unchecked = self.next_unchecked(usize::MAX);
        SpecSummary {
            total: stats.total_tasks,
            checked: stats.checked_tasks,
            unchecked: stats.total_tasks - stats.checked_tasks,
            unchecked_items: unchecked.iter().map(|m| m.text.clone()).collect(),
        }
    }

    // ── Mutations ───────────────────────────────────────────────────────────

    /// Toggle a task's checked state. Returns the new state.
    ///
    /// `task_path` is a chain of indices: `[task_idx]` for top-level,
    /// `[task_idx, subtask_idx]` for one level of nesting, etc.
    ///
    /// Cascading rules:
    /// - Toggling a parent → all descendants get the same state
    /// - Toggling a leaf subtask → ancestors recomputed (checked iff all children checked)
    pub fn toggle_task(&mut self, phase_idx: usize, task_path: &[usize]) -> Result<bool> {
        // Toggle the target task
        let task = self.resolve_task_mut(phase_idx, task_path)?;
        task.checked = !task.checked;
        let new_state = task.checked;

        // Cascade DOWN: set all descendants to the same state
        set_all_checked(task, new_state);

        // Cascade UP: recompute each ancestor's checked state
        // We walk from the deepest ancestor to the shallowest.
        // Each ancestor is checked iff all its children are checked.
        for depth in (0..task_path.len().saturating_sub(1)).rev() {
            let ancestor = self.resolve_task_mut(phase_idx, &task_path[..=depth])?;
            ancestor.checked = ancestor.subtasks.iter().all(|s| s.checked);
        }

        self.roadmap.phases[phase_idx].recompute_status();
        self.dirty = true;
        Ok(new_state)
    }

    /// Update a task's text.
    pub fn update_task_text(
        &mut self,
        phase_idx: usize,
        task_path: &[usize],
        new_text: String,
    ) -> Result<()> {
        let task = self.resolve_task_mut(phase_idx, task_path)?;
        task.text = new_text;
        self.dirty = true;
        Ok(())
    }

    /// Update a task's detail.
    pub fn update_task_detail(
        &mut self,
        phase_idx: usize,
        task_path: &[usize],
        new_detail: String,
    ) -> Result<()> {
        let task = self.resolve_task_mut(phase_idx, task_path)?;
        task.detail = new_detail;
        self.dirty = true;
        Ok(())
    }

    /// Append a new task to a phase.
    pub fn add_task(&mut self, phase_idx: usize, text: String) -> Result<()> {
        let phase = self.roadmap.phases.get_mut(phase_idx).ok_or_else(|| {
            MagistrError::Config(format!("phase index {phase_idx} out of bounds"))
        })?;
        phase.tasks.push(Task {
            text,
            checked: false,
            detail: String::new(),
            subtasks: vec![],
        });
        phase.recompute_status();
        self.dirty = true;
        Ok(())
    }

    /// Add a subtask under an existing task.
    pub fn add_subtask(
        &mut self,
        phase_idx: usize,
        task_path: &[usize],
        text: String,
    ) -> Result<()> {
        let task = self.resolve_task_mut(phase_idx, task_path)?;
        task.subtasks.push(Task {
            text,
            checked: false,
            detail: String::new(),
            subtasks: vec![],
        });
        self.roadmap.phases[phase_idx].recompute_status();
        self.dirty = true;
        Ok(())
    }

    /// Delete a task (and its subtasks).
    pub fn delete_task(&mut self, phase_idx: usize, task_path: &[usize]) -> Result<()> {
        if task_path.is_empty() {
            return Err(MagistrError::Config("empty task path".into()));
        }

        let phase = self.roadmap.phases.get_mut(phase_idx).ok_or_else(|| {
            MagistrError::Config(format!("phase index {phase_idx} out of bounds"))
        })?;

        // Navigate to the parent's task list and remove the target
        let (parent_list_path, target_idx) = task_path.split_at(task_path.len() - 1);
        let target_idx = target_idx[0];

        let task_list = if parent_list_path.is_empty() {
            &mut phase.tasks
        } else {
            let mut current = &mut phase.tasks;
            for &idx in parent_list_path {
                let task = current.get_mut(idx).ok_or_else(|| {
                    MagistrError::Config(format!("task index {idx} out of bounds"))
                })?;
                current = &mut task.subtasks;
            }
            current
        };

        if target_idx >= task_list.len() {
            return Err(MagistrError::Config(format!(
                "task index {target_idx} out of bounds"
            )));
        }
        task_list.remove(target_idx);
        self.roadmap.phases[phase_idx].recompute_status();
        self.dirty = true;
        Ok(())
    }

    /// Swap two tasks within the same phase (for reordering).
    pub fn swap_tasks(&mut self, phase_idx: usize, idx_a: usize, idx_b: usize) -> Result<()> {
        let phase = self.roadmap.phases.get_mut(phase_idx).ok_or_else(|| {
            MagistrError::Config(format!("phase index {phase_idx} out of bounds"))
        })?;
        let len = phase.tasks.len();
        if idx_a >= len || idx_b >= len {
            return Err(MagistrError::Config(format!(
                "task index out of bounds: {idx_a} or {idx_b} >= {len}"
            )));
        }
        phase.tasks.swap(idx_a, idx_b);
        self.dirty = true;
        Ok(())
    }

    /// Move a task from one phase to another.
    pub fn move_task_between_phases(
        &mut self,
        from_phase: usize,
        task_idx: usize,
        to_phase: usize,
    ) -> Result<()> {
        if from_phase == to_phase {
            return Ok(());
        }
        let num_phases = self.roadmap.phases.len();
        if from_phase >= num_phases || to_phase >= num_phases {
            return Err(MagistrError::Config(format!(
                "phase index out of bounds: {from_phase} or {to_phase} >= {num_phases}"
            )));
        }
        let from = &self.roadmap.phases[from_phase];
        if task_idx >= from.tasks.len() {
            return Err(MagistrError::Config(format!(
                "task index {task_idx} out of bounds for phase {from_phase}"
            )));
        }
        let task = self.roadmap.phases[from_phase].tasks.remove(task_idx);
        self.roadmap.phases[to_phase].tasks.push(task);
        self.roadmap.phases[from_phase].recompute_status();
        self.roadmap.phases[to_phase].recompute_status();
        self.dirty = true;
        Ok(())
    }

    /// Append a new phase (or insert before/after another phase).
    ///
    /// If `position` is `None`, appends to the end.
    /// Use `InsertPosition::Before("G27")` or `InsertPosition::After("G26")`
    /// to control placement.
    pub fn add_phase_at(
        &mut self,
        phase: Phase,
        position: Option<InsertPosition<'_>>,
    ) -> Result<()> {
        // Check for duplicate ID
        if self.roadmap.phases.iter().any(|p| p.id == phase.id) {
            return Err(MagistrError::Config(format!(
                "duplicate phase ID: '{}'",
                phase.id
            )));
        }
        match position {
            None => self.roadmap.phases.push(phase),
            Some(pos) => {
                let idx = self.resolve_insert_position(pos)?;
                self.roadmap.phases.insert(idx, phase);
            }
        }
        self.dirty = true;
        Ok(())
    }

    /// Append a new phase.
    pub fn add_phase(&mut self, phase: Phase) -> Result<()> {
        self.add_phase_at(phase, None)
    }

    /// Move a phase to a new position (before or after another phase).
    pub fn move_phase(&mut self, phase_id: &str, position: InsertPosition<'_>) -> Result<()> {
        let src_idx = self
            .roadmap
            .phases
            .iter()
            .position(|p| p.id == phase_id)
            .ok_or_else(|| MagistrError::Config(format!("unknown phase ID: '{phase_id}'")))?;

        // Remove first, then resolve target (indices shift after remove)
        let phase = self.roadmap.phases.remove(src_idx);
        let target_idx = self.resolve_insert_position(position)?;
        self.roadmap.phases.insert(target_idx, phase);
        self.dirty = true;
        Ok(())
    }

    /// Update a phase's metadata fields. Only non-`None` fields are changed.
    #[allow(clippy::too_many_arguments)]
    pub fn update_phase(
        &mut self,
        phase_id: &str,
        name: Option<&str>,
        tagline: Option<Option<&str>>,
        problem: Option<&str>,
        solution: Option<&str>,
        depends_on: Option<&[String]>,
    ) -> Result<()> {
        let phase = self
            .roadmap
            .phases
            .iter_mut()
            .find(|p| p.id == phase_id)
            .ok_or_else(|| MagistrError::Config(format!("unknown phase ID: '{phase_id}'")))?;

        if let Some(n) = name {
            phase.name = n.to_string();
        }
        if let Some(t) = tagline {
            phase.tagline = t.map(str::to_owned);
        }
        if let Some(p) = problem {
            phase.problem = p.to_string();
        }
        if let Some(s) = solution {
            phase.solution = s.to_string();
        }
        if let Some(deps) = depends_on {
            phase.depends_on = deps.to_vec();
        }
        self.dirty = true;
        Ok(())
    }

    /// Remove a phase by index.
    pub fn remove_phase(&mut self, phase_idx: usize) -> Result<()> {
        if phase_idx >= self.roadmap.phases.len() {
            return Err(MagistrError::Config(format!(
                "phase index {phase_idx} out of bounds"
            )));
        }
        self.roadmap.phases.remove(phase_idx);
        self.dirty = true;
        Ok(())
    }

    /// Remove a phase by its ID. Returns the removed phase.
    pub fn remove_phase_by_id(&mut self, phase_id: &str) -> Result<Phase> {
        let idx = self
            .roadmap
            .phases
            .iter()
            .position(|p| p.id == phase_id)
            .ok_or_else(|| MagistrError::Config(format!("unknown phase ID: '{phase_id}'")))?;
        let phase = self.roadmap.phases.remove(idx);
        self.dirty = true;
        Ok(phase)
    }

    /// Resolve an `InsertPosition` to a concrete index in the phases vec.
    fn resolve_insert_position(&self, pos: InsertPosition<'_>) -> Result<usize> {
        match pos {
            InsertPosition::Before(ref_id) => self
                .roadmap
                .phases
                .iter()
                .position(|p| p.id == ref_id)
                .ok_or_else(|| MagistrError::Config(format!("unknown phase ID: '{ref_id}'"))),
            InsertPosition::After(ref_id) => {
                let idx = self
                    .roadmap
                    .phases
                    .iter()
                    .position(|p| p.id == ref_id)
                    .ok_or_else(|| MagistrError::Config(format!("unknown phase ID: '{ref_id}'")))?;
                Ok(idx + 1)
            }
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    /// Resolve a mutable reference to a task by phase index + path.
    fn resolve_task_mut(&mut self, phase_idx: usize, task_path: &[usize]) -> Result<&mut Task> {
        if task_path.is_empty() {
            return Err(MagistrError::Config("empty task path".into()));
        }

        let phase = self.roadmap.phases.get_mut(phase_idx).ok_or_else(|| {
            MagistrError::Config(format!("phase index {phase_idx} out of bounds"))
        })?;

        let mut current_list = &mut phase.tasks;
        for (depth, &idx) in task_path.iter().enumerate() {
            if depth == task_path.len() - 1 {
                return current_list.get_mut(idx).ok_or_else(|| {
                    MagistrError::Config(format!("task index {idx} out of bounds"))
                });
            }
            let task = current_list
                .get_mut(idx)
                .ok_or_else(|| MagistrError::Config(format!("task index {idx} out of bounds")))?;
            current_list = &mut task.subtasks;
        }

        Err(MagistrError::Config("unreachable task path".into()))
    }
}

// ── Cascading helpers ───────────────────────────────────────────────────────

/// Recursively set `checked` on a task and all its descendants.
fn set_all_checked(task: &mut Task, checked: bool) {
    for sub in &mut task.subtasks {
        sub.checked = checked;
        set_all_checked(sub, checked);
    }
}

/// Bottom-up sanitize: derive each parent's `checked` from its children.
/// Call on every task tree after loading to fix contradictions.
fn sanitize_task_tree(task: &mut Task) {
    for sub in &mut task.subtasks {
        sanitize_task_tree(sub);
    }
    if !task.subtasks.is_empty() {
        task.checked = task.subtasks.iter().all(|s| s.checked);
    }
}

// ── Query helpers ───────────────────────────────────────────────────────────

fn collect_unchecked(
    results: &mut Vec<TaskMatch>,
    phase_idx: usize,
    phase_id: &str,
    phase_name: &str,
    tasks: &[Task],
    parent_path: &[usize],
) {
    for (i, task) in tasks.iter().enumerate() {
        let mut path = parent_path.to_vec();
        path.push(i);
        if !task.checked {
            results.push(TaskMatch {
                phase_idx,
                task_path: path.clone(),
                text: task.text.clone(),
                checked: false,
                phase_id: phase_id.to_string(),
                phase_name: phase_name.to_string(),
                task_id: task_id(phase_id, &path),
            });
        }
        collect_unchecked(
            results,
            phase_idx,
            phase_id,
            phase_name,
            &task.subtasks,
            &path,
        );
    }
}

fn search_tasks(
    results: &mut Vec<TaskMatch>,
    phase_idx: usize,
    phase_id: &str,
    phase_name: &str,
    tasks: &[Task],
    parent_path: &[usize],
    query_lower: &str,
) {
    for (i, task) in tasks.iter().enumerate() {
        let mut path = parent_path.to_vec();
        path.push(i);
        if task.text.to_lowercase().contains(query_lower) {
            results.push(TaskMatch {
                phase_idx,
                task_path: path.clone(),
                text: task.text.clone(),
                checked: task.checked,
                phase_id: phase_id.to_string(),
                phase_name: phase_name.to_string(),
                task_id: task_id(phase_id, &path),
            });
        }
        search_tasks(
            results,
            phase_idx,
            phase_id,
            phase_name,
            &task.subtasks,
            &path,
            query_lower,
        );
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "index.tests.rs"]
mod tests;
