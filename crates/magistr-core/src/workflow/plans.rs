//! Plan archive — persistent storage for past and active plans.
//!
//! Saves plans to `.claude/plans/` with metadata (status, timestamps,
//! task name) so they can be listed, resumed, or reviewed later.
//!
//! Each plan is a single TOML file containing both metadata and the
//! plan markdown content as a multi-line string field.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{MagistrError, Result};

/// Directory name for archived plans.
const PLANS_DIR: &str = ".claude/plans";

/// Status of a saved plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanStatus {
    /// Currently being executed (only one at a time).
    Active,
    /// Finished successfully — quality gates passed, committed.
    Completed,
    /// Cancelled or superseded by a new plan.
    Abandoned,
}

impl PlanStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Active => "●",
            Self::Completed => "✓",
            Self::Abandoned => "✗",
        }
    }
}

impl std::fmt::Display for PlanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A saved plan with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPlan {
    /// Unique identifier (timestamp-based slug).
    pub id: String,
    /// When the plan was created.
    pub created_at: DateTime<Utc>,
    /// Current status.
    pub status: PlanStatus,
    /// Short task name / description.
    pub task_name: String,
    /// The full plan content (markdown).
    pub content: String,
    /// Spec items this plan addresses (unchecked item text).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spec_items: Vec<String>,
}

/// Resolve the plans directory for a project.
pub fn plans_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(PLANS_DIR)
}

/// Save a plan to the archive.
///
/// Creates the plans directory if it doesn't exist.
/// Returns the file path where the plan was saved.
pub fn save_plan(project_dir: &Path, plan: &SavedPlan) -> Result<PathBuf> {
    let dir = plans_dir(project_dir);
    std::fs::create_dir_all(&dir).map_err(|e| MagistrError::io(&dir, e))?;

    let filename = format!("{}.toml", plan.id);
    let path = dir.join(&filename);

    let toml_str = toml::to_string_pretty(plan)
        .map_err(|e| MagistrError::Config(format!("TOML error: {e}")))?;

    std::fs::write(&path, toml_str).map_err(|e| MagistrError::io(&path, e))?;

    Ok(path)
}

/// Load a plan from the archive by ID.
pub fn load_plan(project_dir: &Path, id: &str) -> Result<SavedPlan> {
    let path = plans_dir(project_dir).join(format!("{id}.toml"));

    if !path.exists() {
        return Err(MagistrError::Config(format!("plan not found: {id}")));
    }

    let content = std::fs::read_to_string(&path).map_err(|e| MagistrError::io(&path, e))?;

    toml::from_str(&content).map_err(|e| MagistrError::Config(format!("invalid plan TOML: {e}")))
}

/// List all saved plans, sorted by creation time (most recent first).
pub fn list_plans(project_dir: &Path) -> Result<Vec<SavedPlan>> {
    let dir = plans_dir(project_dir);

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut plans = Vec::new();

    let entries = std::fs::read_dir(&dir).map_err(|e| MagistrError::io(&dir, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| MagistrError::io(&dir, e))?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "toml") {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<SavedPlan>(&content) {
                    Ok(plan) => plans.push(plan),
                    Err(_) => continue, // skip malformed files
                },
                Err(_) => continue,
            }
        }
    }

    // Most recent first
    plans.sort_by_key(|p| std::cmp::Reverse(p.created_at));

    Ok(plans)
}

/// Find the most recent active plan.
pub fn find_active_plan(project_dir: &Path) -> Result<Option<SavedPlan>> {
    let plans = list_plans(project_dir)?;
    Ok(plans.into_iter().find(|p| p.status == PlanStatus::Active))
}

/// Update a plan's status in the archive.
pub fn update_status(project_dir: &Path, id: &str, status: PlanStatus) -> Result<()> {
    let mut plan = load_plan(project_dir, id)?;
    plan.status = status;
    save_plan(project_dir, &plan)?;
    Ok(())
}

/// Abandon all currently active plans (used before creating a new one).
pub fn abandon_active_plans(project_dir: &Path) -> Result<u32> {
    let plans = list_plans(project_dir)?;
    let mut count = 0;

    for plan in &plans {
        if plan.status == PlanStatus::Active {
            update_status(project_dir, &plan.id, PlanStatus::Abandoned)?;
            count += 1;
        }
    }

    Ok(count)
}

/// Generate a plan ID from the current timestamp and a task slug.
pub fn generate_id(task_name: &str) -> String {
    let now = Utc::now();
    let timestamp = now.format("%Y%m%d-%H%M%S");
    let slug = slugify(task_name);
    format!("{timestamp}-{slug}")
}

/// Create a SavedPlan from the current plan and task files.
///
/// Reads `.claude/current-plan.md` and `.claude/current-task.md`,
/// creates a new `SavedPlan` with `Active` status.
pub fn archive_current_plan(project_dir: &Path) -> Result<SavedPlan> {
    let plan_path = super::plan::plan_path(project_dir);
    let task_path = super::plan::task_path(project_dir);

    let content =
        std::fs::read_to_string(&plan_path).map_err(|e| MagistrError::io(&plan_path, e))?;

    let task_name = std::fs::read_to_string(&task_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unnamed plan".to_string());

    let id = generate_id(&task_name);

    let plan = SavedPlan {
        id,
        created_at: Utc::now(),
        status: PlanStatus::Active,
        task_name,
        content,
        spec_items: Vec::new(),
    };

    // Abandon any previously active plans
    let _ = abandon_active_plans(project_dir);

    save_plan(project_dir, &plan)?;

    Ok(plan)
}

/// Convert a string to a URL-safe slug.
fn slugify(s: &str) -> String {
    let slug: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse multiple dashes and trim
    let mut result = String::new();
    let mut last_was_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !last_was_dash && !result.is_empty() {
                result.push('-');
            }
            last_was_dash = true;
        } else {
            result.push(c);
            last_was_dash = false;
        }
    }

    // Trim trailing dash and limit length
    let trimmed = result.trim_end_matches('-');
    if trimmed.chars().count() > 40 {
        // Truncate to 40 chars at a char boundary
        let end = trimmed
            .char_indices()
            .nth(40)
            .map(|(i, _)| i)
            .unwrap_or(trimmed.len());
        trimmed[..end].trim_end_matches('-').to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
#[path = "plans.tests.rs"]
mod tests;
