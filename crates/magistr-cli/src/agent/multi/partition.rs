//! Work partitioning strategies — split a task into subtasks for parallel execution.
//!
//! Each strategy takes a high-level task description and produces a set of
//! `SubTask` values that can be assigned to individual workers.

use std::path::PathBuf;

use magistr_core::types::{Archetype, resolve_archetype};
use serde::{Deserialize, Serialize};

// ─── Partition Strategy ────────────────────────────────────────────────────

/// Strategy for splitting work across multiple agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionStrategy {
    /// Split by file — each worker handles a disjoint set of files.
    SplitByFile,
    /// Split by phase — separate workers for research, code, and test.
    SplitByPhase,
    /// Split by component — each worker handles a separate module/crate.
    SplitByComponent,
    /// Split by archetype — each worker handles one language/framework sub-project.
    SplitByArchetype,
}

impl std::fmt::Display for PartitionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SplitByFile => write!(f, "split-by-file"),
            Self::SplitByPhase => write!(f, "split-by-phase"),
            Self::SplitByComponent => write!(f, "split-by-component"),
            Self::SplitByArchetype => write!(f, "split-by-archetype"),
        }
    }
}

// ─── SubTask ───────────────────────────────────────────────────────────────

/// A single unit of work produced by partitioning.
#[derive(Debug, Clone)]
pub struct SubTask {
    /// Unique identifier for this subtask.
    pub task_id: String,
    /// Human-readable description.
    pub description: String,
    /// The prompt to send to the worker agent.
    pub prompt: String,
    /// Files/directories this worker is scoped to.
    pub scope: Vec<PathBuf>,
    /// Which strategy produced this subtask.
    pub strategy: PartitionStrategy,
    /// Archetype ID this subtask is scoped to (for archetype-aware orchestration).
    pub archetype_id: Option<String>,
}

// ─── Auto-Strategy Selection ──────────────────────────────────────────────

/// Analyze the task context and automatically select the best partition strategy.
///
/// Heuristics:
/// - **Multiple components** provided → `SplitByComponent` (each module gets its own worker).
/// - **Many files** (>5) provided → `SplitByFile` (chunk files across workers).
/// - **Research-heavy** keywords detected → `SplitByPhase` (research → implement → test).
/// - **Default** → `SplitByPhase` (safest general-purpose strategy).
#[must_use]
pub fn auto_select_strategy(
    task_description: &str,
    files: &[PathBuf],
    components: &[String],
) -> PartitionStrategy {
    // If multiple components are available, split by component for isolation.
    if components.len() >= 2 {
        return PartitionStrategy::SplitByComponent;
    }

    // If many files are involved, split by file for parallelism.
    if files.len() > 5 {
        return PartitionStrategy::SplitByFile;
    }

    // Keyword analysis for phase-based splitting.
    let desc_lower = task_description.to_lowercase();
    let phase_keywords = [
        "research",
        "investigate",
        "analyze",
        "explore",
        "implement",
        "refactor",
        "add feature",
        "build",
    ];
    let has_phase_signal = phase_keywords.iter().any(|kw| desc_lower.contains(kw));
    if has_phase_signal {
        return PartitionStrategy::SplitByPhase;
    }

    // If a moderate number of files are provided, split by file.
    if files.len() >= 2 {
        return PartitionStrategy::SplitByFile;
    }

    // Default: split by phase (research → implement → test).
    PartitionStrategy::SplitByPhase
}

// ─── Partitioning Functions ────────────────────────────────────────────────

/// Partition a task into subtasks based on the given strategy.
///
/// - `task_description`: The high-level task to split.
/// - `files`: Files involved in the task (for file-based splitting).
/// - `components`: Component/module names (for component-based splitting).
/// - `strategy`: Which partitioning strategy to use.
pub fn partition(
    task_description: &str,
    files: &[PathBuf],
    components: &[String],
    strategy: &PartitionStrategy,
) -> Vec<SubTask> {
    match strategy {
        PartitionStrategy::SplitByFile => partition_by_file(task_description, files),
        PartitionStrategy::SplitByPhase => partition_by_phase(task_description),
        PartitionStrategy::SplitByComponent => partition_by_component(task_description, components),
        PartitionStrategy::SplitByArchetype => {
            // SplitByArchetype requires archetypes — fall back to component split.
            partition_by_component(task_description, components)
        }
    }
}

/// Partition a task by archetype — one subtask per archetype.
///
/// Each archetype gets its own subtask scoped to its root directory. If files
/// are provided, they're routed to the archetype that owns them via
/// `resolve_archetype()`. Files that don't match any archetype are assigned
/// to the first (root) archetype.
pub fn partition_by_archetype(
    task_description: &str,
    archetypes: &[Archetype],
    files: &[PathBuf],
) -> Vec<SubTask> {
    if archetypes.is_empty() {
        return vec![SubTask {
            task_id: "archetype-all".into(),
            description: task_description.into(),
            prompt: task_description.into(),
            scope: vec![],
            strategy: PartitionStrategy::SplitByArchetype,
            archetype_id: None,
        }];
    }

    // Route files to archetypes.
    let mut archetype_files: std::collections::HashMap<&str, Vec<PathBuf>> =
        std::collections::HashMap::new();
    for file in files {
        if let Some(arch) = resolve_archetype(archetypes, file) {
            archetype_files
                .entry(&arch.id)
                .or_default()
                .push(file.clone());
        }
    }

    archetypes
        .iter()
        .map(|arch| {
            let scope_files = archetype_files.remove(arch.id.as_str()).unwrap_or_default();
            let scope = if scope_files.is_empty() {
                vec![arch.root.clone()]
            } else {
                scope_files
            };

            SubTask {
                task_id: format!("archetype-{}", arch.id),
                description: format!(
                    "{} ({}/{})",
                    arch.name,
                    arch.language,
                    arch.framework.as_deref().unwrap_or("none")
                ),
                prompt: format!(
                    "You are working on the **{name}** ({lang}) sub-project.\n\
                     Root directory: `{root}`\n\n\
                     Only modify files under `{root}`.\n\n\
                     Task: {task}",
                    name = arch.name,
                    lang = arch.language,
                    root = arch.root.display(),
                    task = task_description,
                ),
                scope,
                strategy: PartitionStrategy::SplitByArchetype,
                archetype_id: Some(arch.id.clone()),
            }
        })
        .collect()
}

/// Partition with auto-selected strategy.
///
/// Convenience function that first selects the best strategy, then partitions.
pub fn auto_partition(
    task_description: &str,
    files: &[PathBuf],
    components: &[String],
) -> Vec<SubTask> {
    let strategy = auto_select_strategy(task_description, files, components);
    partition(task_description, files, components, &strategy)
}

/// Split by file — each subtask handles a subset of files.
///
/// Groups files into chunks of up to 5 files per worker to keep each
/// worker's scope manageable.
fn partition_by_file(task_description: &str, files: &[PathBuf]) -> Vec<SubTask> {
    if files.is_empty() {
        return vec![SubTask {
            task_id: "file-all".into(),
            description: task_description.into(),
            prompt: task_description.into(),
            scope: vec![],
            strategy: PartitionStrategy::SplitByFile,
            archetype_id: None,
        }];
    }

    const MAX_FILES_PER_WORKER: usize = 5;
    files
        .chunks(MAX_FILES_PER_WORKER)
        .enumerate()
        .map(|(i, chunk)| {
            let file_list = chunk
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            SubTask {
                task_id: format!("file-{i}"),
                description: format!("Handle files: {file_list}"),
                prompt: format!(
                    "{task_description}\n\nScope: Only modify these files:\n{file_list}"
                ),
                scope: chunk.to_vec(),
                strategy: PartitionStrategy::SplitByFile,
                archetype_id: None,
            }
        })
        .collect()
}

/// Split by phase — separate workers for research, implementation, and testing.
fn partition_by_phase(task_description: &str) -> Vec<SubTask> {
    vec![
        SubTask {
            task_id: "phase-research".into(),
            description: "Research phase".into(),
            prompt: format!(
                "RESEARCH ONLY — do not modify any files.\n\n\
                 Investigate the following task and produce a detailed analysis \
                 with findings, relevant code locations, and recommendations:\n\n\
                 {task_description}"
            ),
            scope: vec![],
            strategy: PartitionStrategy::SplitByPhase,
            archetype_id: None,
        },
        SubTask {
            task_id: "phase-implement".into(),
            description: "Implementation phase".into(),
            prompt: format!(
                "IMPLEMENT the following task. Write production code only \
                 (no tests in this step):\n\n{task_description}"
            ),
            scope: vec![],
            strategy: PartitionStrategy::SplitByPhase,
            archetype_id: None,
        },
        SubTask {
            task_id: "phase-test".into(),
            description: "Testing phase".into(),
            prompt: format!(
                "WRITE TESTS for the following task. Focus on unit tests and \
                 edge cases. Do not modify production code:\n\n{task_description}"
            ),
            scope: vec![],
            strategy: PartitionStrategy::SplitByPhase,
            archetype_id: None,
        },
    ]
}

/// Split by component — each subtask handles a separate module or crate.
fn partition_by_component(task_description: &str, components: &[String]) -> Vec<SubTask> {
    if components.is_empty() {
        return vec![SubTask {
            task_id: "component-all".into(),
            description: task_description.into(),
            prompt: task_description.into(),
            scope: vec![],
            strategy: PartitionStrategy::SplitByComponent,
            archetype_id: None,
        }];
    }

    components
        .iter()
        .enumerate()
        .map(|(i, component)| SubTask {
            task_id: format!("component-{i}"),
            description: format!("Handle component: {component}"),
            prompt: format!(
                "{task_description}\n\nScope: Only modify code in the `{component}` module/crate."
            ),
            scope: vec![PathBuf::from(component)],
            strategy: PartitionStrategy::SplitByComponent,
            archetype_id: None,
        })
        .collect()
}

#[cfg(test)]
#[path = "partition.tests.rs"]
mod tests;
