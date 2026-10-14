//! Context injection for ACP agent sessions.
//!
//! `ContextBuilder` assembles project context (recent commits, spec status,
//! working tree state, active plan) into plain text for system prompt injection
//! and compaction recovery.
//!
//! This replaces the `reinject-context.sh` SessionStart hook with a typed,
//! testable, backend-agnostic Rust implementation.

use std::path::PathBuf;
use std::process::Command;

use crate::error::{MagistrError, Result};
use crate::workflow::project_context::ProjectContext;
use crate::workflow::spec;

/// Builds project context for injection into agent sessions.
///
/// Produces plain text output suitable for the ACP backend.
/// Does not produce JSON or any agent-specific format.
pub struct ContextBuilder {
    project_dir: PathBuf,
    spec_path: PathBuf,
    max_commits: usize,
    /// The think-and-ship reader on a native project: spec progress then
    /// comes from chunks and `spec_path` is never opened.
    native: Option<magistr_engine::NativeReader>,
}

impl ContextBuilder {
    /// Create a new context builder for the given project.
    pub fn new(project_dir: impl Into<PathBuf>, spec_path: impl Into<PathBuf>) -> Self {
        Self {
            project_dir: project_dir.into(),
            spec_path: spec_path.into(),
            max_commits: 5,
            native: None,
        }
    }

    /// Read spec progress from the think-and-ship reader instead of
    /// `spec_path` (open it before raw mode).
    #[must_use]
    pub fn with_native(mut self, engines: Option<magistr_engine::NativeReader>) -> Self {
        self.native = engines;
        self
    }

    /// Set the maximum number of recent commits to include.
    #[must_use]
    pub fn max_commits(mut self, n: usize) -> Self {
        self.max_commits = n;
        self
    }

    /// Gather all project context.
    pub fn gather(&self) -> Result<ProjectContext> {
        let summary = self.spec_summary()?;
        let (total, checked, unchecked) = match summary {
            Some(s) => (s.total, s.checked, s.unchecked),
            None => (0, 0, 0),
        };
        Ok(ProjectContext {
            recent_commits: self.git_log(),
            spec_total: total,
            spec_checked: checked,
            spec_unchecked: unchecked,
            dirty_files: self.dirty_files(),
            active_plan: self.read_plan(),
            active_task: self.read_task(),
        })
    }

    /// Build full context string for system prompt injection.
    ///
    /// Returns a human-readable plain text summary of the project state.
    pub fn build(&self) -> Result<String> {
        let ctx = self.gather()?;
        Ok(self.format_full(&ctx))
    }

    /// Build a shorter context string for compaction recovery.
    ///
    /// After context compaction, this provides essential state without
    /// the full detail of `build()`.
    pub fn build_compaction_recovery(&self) -> Result<String> {
        let ctx = self.gather()?;
        Ok(self.format_recovery(&ctx))
    }

    // ── Private helpers ─────────────────────────────────────────────────

    fn git_log(&self) -> Vec<String> {
        let output = Command::new("git")
            .args(["log", "--oneline", &format!("-{}", self.max_commits)])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(String::from)
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn dirty_files(&self) -> Vec<String> {
        let output = Command::new("git")
            .args(["status", "--porcelain", "--no-renames"])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| {
                        // Format: "XY filename" — extract just the filename
                        if l.len() > 3 {
                            l[3..].to_string()
                        } else {
                            l.to_string()
                        }
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn read_plan(&self) -> Option<String> {
        let plan_path = crate::paths::plan(&self.project_dir);
        let content = std::fs::read_to_string(plan_path).ok()?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(trimmed.to_string())
    }

    fn read_task(&self) -> Option<String> {
        let task_path = crate::paths::task(&self.project_dir);
        let content = std::fs::read_to_string(task_path).ok()?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(trimmed.to_string())
    }

    fn spec_summary(&self) -> Result<Option<spec::SpecSummary>> {
        if let Some(engines) = &self.native {
            let progress = engines.roadmap_progress().map_err(MagistrError::Workflow)?;
            // Context describes all unfinished work, not just the candidate
            // an execution selector may admit for the next attempt.
            return Ok(Some(spec::SpecSummary {
                total: progress.total_tasks as usize,
                checked: progress.checked_tasks as usize,
                unchecked: progress.total_tasks.saturating_sub(progress.checked_tasks) as usize,
                unchecked_items: Vec::new(),
            }));
        }
        Ok(spec::spec_summary_from_project(&self.project_dir, &self.spec_path).ok())
    }

    fn format_full(&self, ctx: &ProjectContext) -> String {
        let mut out = String::with_capacity(2048);

        out.push_str("# Project State\n\n");

        // Spec progress
        if ctx.spec_total > 0 {
            let progress = if ctx.spec_total > 0 {
                (ctx.spec_checked as f64 / ctx.spec_total as f64) * 100.0
            } else {
                0.0
            };
            out.push_str("## Spec Progress\n\n");
            out.push_str(&format!(
                "- **Progress:** {}/{} items complete ({:.0}%)\n",
                ctx.spec_checked, ctx.spec_total, progress
            ));
            out.push_str(&format!("- **Remaining:** {} items\n", ctx.spec_unchecked));
            out.push('\n');
        }

        // Recent commits
        if !ctx.recent_commits.is_empty() {
            out.push_str("## Recent Commits\n\n");
            for commit in &ctx.recent_commits {
                out.push_str(&format!("- `{commit}`\n"));
            }
            out.push('\n');
        }

        // Working tree state
        if !ctx.dirty_files.is_empty() {
            out.push_str("## Uncommitted Changes\n\n");
            for file in &ctx.dirty_files {
                out.push_str(&format!("- {file}\n"));
            }
            out.push('\n');
        }

        // Active task
        if let Some(ref task) = ctx.active_task {
            out.push_str(&format!("## Active Task\n\n{task}\n\n"));
        }

        // Active plan
        if let Some(ref plan) = ctx.active_plan {
            out.push_str(&format!("## Active Plan\n\n{plan}\n\n"));
        }

        out
    }

    fn format_recovery(&self, ctx: &ProjectContext) -> String {
        let mut out = String::with_capacity(512);

        out.push_str("# Context Recovery (post-compaction)\n\n");
        out.push_str("The context window was compacted. Here is the essential project state:\n\n");

        // Spec progress (essential)
        if ctx.spec_total > 0 {
            let progress = if ctx.spec_total > 0 {
                (ctx.spec_checked as f64 / ctx.spec_total as f64) * 100.0
            } else {
                0.0
            };
            out.push_str(&format!(
                "- Spec: {}/{} complete ({:.0}%), {} remaining\n",
                ctx.spec_checked, ctx.spec_total, progress, ctx.spec_unchecked
            ));
        }

        // Last commit only
        if let Some(last) = ctx.recent_commits.first() {
            out.push_str(&format!("- Last commit: `{last}`\n"));
        }

        // Dirty file count
        if !ctx.dirty_files.is_empty() {
            out.push_str(&format!("- Uncommitted files: {}\n", ctx.dirty_files.len()));
        }

        // Active task (essential for resumption)
        if let Some(ref task) = ctx.active_task {
            out.push_str(&format!("- Active task: {task}\n"));
        }

        out.push_str("\nRefer to the spec file and plan file for full details.\n");

        out
    }
}

#[cfg(test)]
#[path = "context_builder.tests.rs"]
mod tests;
