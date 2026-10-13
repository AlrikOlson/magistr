//! Gathering the home page's project list — the projects this machine has
//! run sessions in, read from the session database's recorded directories.
//! No second record is kept: a project is recent because a session said so.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use magistr_cockpit::cockpit::{GalaxyData, GalaxyProject};

use crate::config::inputs::ProjectInputs;
use crate::session::session_filter::SessionFilter;
use crate::session::session_record::SessionRecord;

/// How many other projects the home page lists beside the open one.
const RECENT_PROJECTS: usize = 12;
/// How far back the session database is read for them.
const RECENT_SESSIONS: usize = 500;

/// The home page's projects for a window rooted on `here`; an unlocated or
/// unopenable session database lists this project alone, never an error.
pub(crate) fn collect(here: &Path, inputs: &ProjectInputs) -> GalaxyData {
    let records = crate::session::storage::sessions_db_path(inputs)
        .and_then(|path| crate::session::storage::open_db(&path).ok())
        .and_then(|conn| {
            let filter = SessionFilter::new().with_limit(RECENT_SESSIONS);
            crate::session::storage::query_sessions(&conn, &filter).ok()
        })
        .unwrap_or_default();
    from_records(here, &records, Utc::now())
}

/// `records` as the project list at `now`: the open project leads, then each
/// recorded directory that still holds a Magistr project, once, by its
/// latest session, newest first.
fn from_records(here: &Path, records: &[SessionRecord], now: DateTime<Utc>) -> GalaxyData {
    let here = here.canonicalize().unwrap_or_else(|_| here.to_path_buf());
    let mut records: Vec<&SessionRecord> = records.iter().collect();
    records.sort_by_key(|record| std::cmp::Reverse(record.started_at));
    let mut projects = vec![project(&here, true, None)];
    let mut seen = vec![here.clone()];
    let mut spent_cents = 0u64;
    for record in records {
        let Some(dir) = recorded_project(&record.project) else {
            continue;
        };
        let known = seen.contains(&dir);
        if !known && seen.len() > RECENT_PROJECTS {
            continue;
        }
        spent_cents += cents(record.cost_usd);
        if dir == here {
            let idle = &mut projects[0].idle_secs;
            *idle = idle.or_else(|| idle_secs(record, now));
        } else if !known {
            projects.push(project(&dir, false, idle_secs(record, now)));
            seen.push(dir);
        }
    }
    GalaxyData {
        projects,
        spent_cents,
        cap_cents: None,
        stop_cents: None,
    }
}

/// The project's name as every card and cluster spells it: the config's,
/// or — without one, or with an empty one — the directory's own.
pub(crate) fn project_name(dir: &Path) -> String {
    crate::paths::project_json(dir)
        .is_ok_and(|path| path.exists())
        .then(|| crate::validate::load_project_config(dir).ok())
        .flatten()
        .map(|c| c.project.name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| directory_name(dir))
}

/// The directory's own name, the fallback project name.
pub(crate) fn directory_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// The directory a session recorded, when it still holds a Magistr project.
fn recorded_project(recorded: &str) -> Option<PathBuf> {
    let path = Path::new(recorded);
    if !path.is_absolute() {
        return None;
    }
    let dir = path.canonicalize().ok()?;
    crate::paths::project_json(&dir)
        .ok()?
        .is_file()
        .then_some(dir)
}

fn project(dir: &Path, here: bool, idle_secs: Option<u64>) -> GalaxyProject {
    GalaxyProject {
        name: project_name(dir),
        dir: Some(dir.display().to_string()),
        here,
        idle_secs,
        ..GalaxyProject::default()
    }
}

fn idle_secs(record: &SessionRecord, now: DateTime<Utc>) -> Option<u64> {
    u64::try_from((now - record.started_at).num_seconds()).ok()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a session's cost is small and never negative once clamped
fn cents(usd: f64) -> u64 {
    (usd.max(0.0) * 100.0).round() as u64
}

#[cfg(test)]
#[path = "galaxy_data.tests.rs"]
mod tests;
