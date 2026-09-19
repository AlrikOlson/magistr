//! Gathering the launcher's card data — the one job bare `magistr`
//! spawns before taking the terminal over. Everything expensive lives
//! here, off the cockpit's loop thread: the project config read, four
//! git subprocesses, the backend PATH scan and the session database.

use std::path::Path;

use magistr_cockpit::cockpit::{LauncherData, LauncherSession, SessionOutcome};

use crate::config::inputs::ProjectInputs;

/// How many recent sessions the launcher lists.
const RECENT_SESSIONS: usize = 5;

/// The launcher's card data for the project at `dir`; the session
/// database is found through the captured `inputs`.
pub(crate) fn collect(dir: &Path, inputs: &ProjectInputs) -> LauncherData {
    let has_config = crate::paths::project_json(dir).exists();
    let config = has_config
        .then(|| crate::validate::load_project_config(dir).ok())
        .flatten();
    let project = super::galaxy_data::project_name(dir);
    let stack = config.as_ref().map_or_else(
        || "not detected".to_owned(),
        |c| {
            stack_summary(
                &c.stack.language,
                &c.stack.framework,
                &c.stack.package_manager,
            )
        },
    );
    LauncherData {
        project,
        has_config,
        stack,
        git: git_summary(dir),
        backend: backend_summary(),
        sessions: recent_sessions(dir, inputs),
    }
}

/// `Rust / Cargo`-style summary, skipping unknown parts.
fn stack_summary(language: &str, framework: &str, package_manager: &str) -> String {
    let parts: Vec<&str> = [language, framework, package_manager]
        .into_iter()
        .filter(|part| !part.is_empty() && *part != "unknown" && *part != "none")
        .collect();
    if parts.is_empty() {
        "not detected".to_owned()
    } else {
        parts.join(" / ")
    }
}

/// `master, clean, +1/-2` — or `not a git repository`.
fn git_summary(dir: &Path) -> String {
    let Some(branch) = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]) else {
        return "not a git repository".to_owned();
    };
    let dirty = git(dir, &["status", "--porcelain"])
        .map(|out| out.lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0);
    let mut parts = vec![branch];
    if dirty > 0 {
        parts.push(format!("{dirty} dirty"));
    } else {
        parts.push("clean".to_owned());
    }
    if let Some((ahead, behind)) = ahead_behind(dir)
        && (ahead > 0 || behind > 0)
    {
        parts.push(format!("+{ahead}/-{behind}"));
    }
    parts.join(", ")
}

/// Commits ahead/behind the upstream, when one is set.
fn ahead_behind(dir: &Path) -> Option<(u32, u32)> {
    let out = git(
        dir,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
    )?;
    let (ahead, behind) = out.trim().split_once('\t')?;
    Some((ahead.parse().ok()?, behind.parse().ok()?))
}

/// One git subprocess with quiet stderr (inherited stderr would land on
/// the alternate screen); `None` on any failure. Shared with the
/// deliverables shelf's collector, which reads the same repository.
pub(crate) fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// `ACP (available)` — or `no backend found`.
fn backend_summary() -> String {
    crate::agent::detect_backend().map_or_else(
        || "no backend found".to_owned(),
        |backend| format!("{} (available)", backend.label()),
    )
}

/// The last few sessions for this project from the session database,
/// pre-formatted; an unopenable database is an empty list, not an error.
/// A record the database kept no session id for carries none here
/// either — the launcher draws it and refuses to resume it.
fn recent_sessions(dir: &Path, inputs: &ProjectInputs) -> Vec<LauncherSession> {
    let Some(db_path) = crate::session::storage::sessions_db_path(inputs) else {
        return Vec::new();
    };
    let Ok(conn) = crate::session::storage::open_db(&db_path) else {
        return Vec::new();
    };
    let filter = crate::session::session_filter::SessionFilter::new()
        .with_project(super::galaxy_data::directory_name(dir))
        .with_limit(RECENT_SESSIONS);
    let Ok(records) = crate::session::storage::query_sessions(&conn, &filter) else {
        return Vec::new();
    };
    records
        .into_iter()
        .map(|record| {
            let outcome = if record.tags.iter().any(|t| t == "cancelled") {
                SessionOutcome::Cancelled
            } else if record.tags.iter().any(|t| t == "error") {
                SessionOutcome::Error
            } else {
                SessionOutcome::Success
            };
            let preview: String = record
                .summary
                .as_deref()
                .unwrap_or("(no summary)")
                .chars()
                .take(60)
                .collect();
            LauncherSession {
                id: (!record.session_id.is_empty()).then(|| record.session_id.clone()),
                when: record.started_at.format("%b %d %H:%M").to_string(),
                duration: record.duration_display(),
                outcome,
                preview,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "home_data.tests.rs"]
mod tests;
