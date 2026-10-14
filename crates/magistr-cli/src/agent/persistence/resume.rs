//! Smart session resume — context for resuming a previous session.
//!
//! When a user resumes a session, this module builds a summary of:
//! - What happened in the previous session (turns, cost, files changed)
//! - What changed in the project since the session ended (new commits)
//! - Options: continue where you left off vs start fresh with context

use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Utc};

use super::commit_info::CommitInfo;
use super::session_record::SessionRecord;

/// Admit a saved conversation through the provider that actually owns it.
pub(crate) fn saved_config(
    mut config: crate::agent::AgentConfig,
    id: &str,
) -> crate::error::Result<crate::agent::AgentConfig> {
    use crate::error::MagistrError;
    let path = super::storage::sessions_db_path(config.inputs()).ok_or_else(|| {
        MagistrError::Config("No directory is available for saved sessions.".into())
    })?;
    let conn = super::storage::open_readonly_db(&path)?;
    let record = super::storage::get_session(&conn, id)?
        .ok_or_else(|| MagistrError::Config(format!("Saved session {id} was not found.")))?;
    let root = config
        .cwd
        .canonicalize()
        .map_err(|error| MagistrError::io(&config.cwd, error))?;
    if root != Path::new(&record.workspace) {
        return Err(MagistrError::Config(format!(
            "Open {} to resume this conversation.",
            record.workspace
        )));
    }
    if !record.can_resume() {
        return Err(MagistrError::Config(format!(
            "{} cannot resume this saved conversation.",
            record.provider
        )));
    }
    if config
        .provider
        .as_ref()
        .is_some_and(|provider| provider != &record.provider)
    {
        return Err(MagistrError::Config(format!(
            "This conversation belongs to {}. Resume it with that agent.",
            record.provider
        )));
    }
    config.resume.clone_from(&record.provider_session_id);
    config.provider = Some(record.provider.clone());
    config.model.clone_from(&record.model);
    config.history = super::SessionHistory::from_record(record);
    Ok(config)
}

/// Context for resuming a previous session.
///
/// Built from the last session record and current git state,
/// this provides all the information needed for a resume summary card.
#[derive(Debug, Clone, PartialEq)]
pub struct ResumeContext {
    /// The session being resumed.
    pub session: SessionRecord,
    /// Git commits made since the session ended.
    pub commits_since: Vec<CommitInfo>,
    /// Files modified since the session ended (from git status/log).
    pub files_changed_since: Vec<String>,
    /// Whether there are uncommitted changes in the working tree.
    pub has_dirty_working_tree: bool,
}

impl ResumeContext {
    /// Build resume context from a session record and project directory.
    ///
    /// Queries git for commits and changes since the session ended.
    /// If git is unavailable or the directory is not a repo, git fields are empty.
    pub fn build(session: SessionRecord, project_dir: &Path) -> Self {
        let commits_since = git_commits_since(project_dir, &session.updated_at);
        let files_changed_since = git_files_changed_since(project_dir, &session.updated_at);
        let has_dirty_working_tree = has_dirty_tree(project_dir);

        Self {
            session,
            commits_since,
            files_changed_since,
            has_dirty_working_tree,
        }
    }

    /// Build resume context without git queries (for testing or non-git projects).
    pub fn from_session(session: SessionRecord) -> Self {
        Self {
            session,
            commits_since: Vec::new(),
            files_changed_since: Vec::new(),
            has_dirty_working_tree: false,
        }
    }

    /// Whether there have been changes since the session ended.
    #[must_use]
    pub fn has_changes_since(&self) -> bool {
        !self.commits_since.is_empty()
            || !self.files_changed_since.is_empty()
            || self.has_dirty_working_tree
    }

    /// Format a human-readable summary card for the resume screen.
    #[must_use]
    pub fn summary(&self) -> String {
        let s = &self.session;
        let mut lines = Vec::new();

        lines.push(format!("Session: {}", truncate_id(&s.session_id)));
        lines.push(format!("Project: {}", s.workspace));
        lines.push(format!(
            "Duration: {} | Cost: {} | Turns: {}",
            s.duration_display(),
            s.cost_display(),
            s.turns_display()
        ));

        if !s.files_changed.is_empty() {
            lines.push(format!("Files changed: {}", s.files_changed.len()));
        }

        if let Some(ref summary) = s.summary {
            lines.push(String::new());
            lines.push(format!("Last activity: {summary}"));
        }

        if !s.tags.is_empty() {
            lines.push(format!("Tags: {}", s.tags.join(", ")));
        }

        if self.has_changes_since() {
            lines.push(String::new());
            lines.push("Changes since session ended:".to_string());
            if !self.commits_since.is_empty() {
                lines.push(format!("  {} new commit(s):", self.commits_since.len()));
                for c in self.commits_since.iter().take(5) {
                    lines.push(format!("    {} {}", c.sha, c.subject));
                }
                if self.commits_since.len() > 5 {
                    lines.push(format!("    ... and {} more", self.commits_since.len() - 5));
                }
            }
            if !self.files_changed_since.is_empty() {
                lines.push(format!(
                    "  {} file(s) changed since session",
                    self.files_changed_since.len()
                ));
            }
            if self.has_dirty_working_tree {
                lines.push("  ⚠ Uncommitted changes in working tree".to_string());
            }
        }

        lines.join("\n")
    }
}

/// Truncate a session ID for display (first 8 chars).
fn truncate_id(id: &str) -> &str {
    if id.len() > 8 { &id[..8] } else { id }
}

/// Get git commits since a given timestamp.
fn git_commits_since(project_dir: &Path, since: &DateTime<Utc>) -> Vec<CommitInfo> {
    let since_str = since.format("%Y-%m-%dT%H:%M:%S").to_string();
    let output = Command::new("git")
        .args([
            "log",
            "--oneline",
            "--since",
            &since_str,
            "--format=%h|%s|%ci",
        ])
        .current_dir(project_dir)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .filter(|l| !l.is_empty())
                .filter_map(|line| {
                    let parts: Vec<&str> = line.splitn(3, '|').collect();
                    if parts.len() >= 2 {
                        Some(CommitInfo {
                            sha: parts[0].to_string(),
                            subject: parts[1].to_string(),
                            date: parts.get(2).unwrap_or(&"").to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Get files changed since a given timestamp (from git log).
fn git_files_changed_since(project_dir: &Path, since: &DateTime<Utc>) -> Vec<String> {
    let since_str = since.format("%Y-%m-%dT%H:%M:%S").to_string();
    let output = Command::new("git")
        .args([
            "log",
            "--since",
            &since_str,
            "--name-only",
            "--format=",
            "--diff-filter=ACMR",
        ])
        .current_dir(project_dir)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut files: Vec<String> = stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
            files.sort();
            files.dedup();
            files
        }
        _ => Vec::new(),
    }
}

/// Check if the working tree has uncommitted changes.
fn has_dirty_tree(project_dir: &Path) -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_dir)
        .output()
        .map(|out| out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "resume.tests.rs"]
mod tests;
