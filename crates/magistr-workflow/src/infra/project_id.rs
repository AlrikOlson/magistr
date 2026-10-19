//! Project identity derived from the current working directory.
//!
//! Two co-deployed callers in the same directory must produce the same
//! `project_id` so persisted state and live broadcasts correlate.

use std::env;
use std::path::Path;

pub const PROJECT_SEP: &str = "__";

const SHARED_OVERRIDE_VAR: &str = "MAGISTR_PROJECT_ID";

/// The canonical project settings, shared with setup and project configuration.
pub const PROJECT_DIR: &str = ".magistr";
pub const PROJECT_FILE: &str = "project.json";

/// Where a project's identity came from. Reported by `status`, because
/// "this is the project" and "this is the project BECAUSE a file says so" are
/// different facts and the difference is what a user needs when one repository
/// answers two ways from two directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdSource {
    /// An environment variable. A user who set one meant it.
    Environment,
    /// A committed `.magistr/project.json`, found by walking up.
    RepoFile,
    /// `<basename>-<fnv1a_6hex(cwd)>` — the answer when nothing declares one.
    DerivedFromPath,
}

impl IdSource {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Environment => "the environment",
            Self::RepoFile => "this repository's project file",
            Self::DerivedFromPath => "this directory's path",
        }
    }
}

/// Identity read from the `project` section of Magistr's project settings.
/// Connection credentials remain in machine-local storage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectIdentity {
    /// The machine identity every store is keyed by.
    pub id: String,
    /// What to show a human. Optional; absence means "derive it".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Find the identity file by walking UP from `start`, the way git finds `.git`.
///
/// `start` is a parameter rather than a read of the current directory because
/// `env::current_dir` is process-global: a test that changed it would race every
/// other test in the binary, and this lane has already shipped two gates that
/// agreed with whoever ran them instead of testing anything.
#[must_use]
pub fn find_project_file(start: &Path) -> Option<std::path::PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join(PROJECT_DIR).join(PROJECT_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

/// Read the declared identity for `start`, if the repository declares one.
#[must_use]
pub fn declared_identity_in(start: &Path) -> Option<ProjectIdentity> {
    let path = find_project_file(start)?;
    let text = std::fs::read_to_string(path).ok()?;
    #[derive(serde::Deserialize)]
    struct ProjectDocument {
        project: ProjectIdentity,
    }
    let declared = serde_json::from_str::<ProjectDocument>(&text).ok()?.project;
    let id = sanitize_project_name(declared.id.trim());
    if id.is_empty() {
        return None;
    }
    Some(ProjectIdentity {
        id,
        name: declared
            .name
            .map(|n| sanitize_project_name(n.trim()))
            .filter(|n| !n.is_empty()),
    })
}

/// Resolve the canonical project identifier.
///
/// Order of precedence:
///   1. `server_env_var` (caller-supplied override).
///   2. `MAGISTR_PROJECT_ID` (shared override).
///   3. `.magistr/project.json`, found by walking up from the cwd.
///   4. `<basename>-<fnv1a_6hex(cwd)>` (deterministic fallback).
///
/// An unconfigured directory gets a deterministic identity until setup pins it.
pub fn resolve_project_id(server_env_var: Option<&str>) -> String {
    resolve_project_id_with(server_env_var, env::current_dir().ok().as_deref()).0
}

/// [`resolve_project_id`] with the starting directory supplied, returning WHERE
/// the answer came from as well as what it is.
pub fn resolve_project_id_with(
    server_env_var: Option<&str>,
    cwd: Option<&Path>,
) -> (String, IdSource) {
    let project_name = server_env_var
        .into_iter()
        .chain([SHARED_OVERRIDE_VAR])
        .filter_map(|var| env::var(var).ok())
        .map(|raw| sanitize_project_name(raw.trim()))
        .find(|name| !name.is_empty());
    if let Some(cwd) = cwd {
        return super::resolve_project_id_from(project_name.as_deref(), cwd);
    }
    project_name.map_or_else(
        || ("auto".to_string(), IdSource::DerivedFromPath),
        |name| (name, IdSource::Environment),
    )
}

/// Project id derived purely from a filesystem path (no env overrides):
/// `<basename>-<fnv1a_6hex(path)>`. This is the cwd half of
/// [`resolve_project_id`], exposed so persisted records that carry their
/// recording cwd (e.g. think steps) can be attributed to a project later —
/// the path is hashed as given, NOT canonicalized, because stored cwds were
/// canonicalized at record time and may no longer exist on disk.
pub fn project_id_for_path(path: &Path) -> String {
    let basename = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(sanitize_project_name)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let basename: String = basename.chars().take(24).collect();
    let hash = path_hash6(path);
    format!("{basename}-{hash}")
}

/// The human-readable half of the project's identity: the directory basename
/// with no hash suffix — `example`, without the path hash.
///
/// This is what outward-facing surfaces (a tracker initiative's name) should
/// show a human; [`resolve_project_id`] stays the machine identity. Honours
/// the same env overrides, because a project that overrode its name meant it
/// everywhere. `None` when nothing usable can be derived — the caller decides
/// what absence means, rather than inheriting an "auto" nobody chose.
pub fn project_display_name() -> Option<String> {
    for var in [SHARED_OVERRIDE_VAR] {
        if let Ok(raw) = env::var(var) {
            let sanitized = sanitize_project_name(raw.trim());
            if !sanitized.is_empty() {
                return Some(sanitized);
            }
        }
    }
    let cwd = env::current_dir().ok()?;
    let path = cwd.canonicalize().unwrap_or(cwd);
    // A repository that declared a display name meant that one, and a
    // subdirectory of it must not answer with its own basename.
    if let Some(declared) = declared_identity_in(&path)
        && let Some(name) = declared.name
    {
        return Some(name);
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(sanitize_project_name)?;
    (!name.is_empty()).then_some(name)
}

/// Namespace a caller-supplied session id within the current project.
/// Idempotent: already-prefixed ids pass through unchanged.
pub fn namespace_session_id(project_id: &str, session_id: &str) -> String {
    if session_id.starts_with(project_id) {
        session_id.to_string()
    } else {
        format!("{project_id}{PROJECT_SEP}{session_id}")
    }
}

/// FNV-1a 64-bit, truncated to 24 bits, formatted as 6 hex chars.
/// Deterministic across runs (unlike `DefaultHasher`, which is randomized).
fn path_hash6(path: &Path) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let bytes = path.as_os_str().as_encoded_bytes();
    let mut h: u64 = FNV_OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    format!("{:06x}", (h & 0xff_ffff) as u32)
}

/// Reduce an arbitrary string to `[a-z0-9_.-]`, collapse runs of replaced
/// chars to a single `-`, trim leading/trailing separators, cap at 32 chars.
pub(crate) fn sanitize_project_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_was_replace = false;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.') {
            out.push(c.to_ascii_lowercase());
            last_was_replace = false;
        } else if !last_was_replace && !out.is_empty() {
            out.push('-');
            last_was_replace = true;
        }
    }
    let trimmed = out.trim_matches(|c: char| c == '-' || c == '.');
    let capped: String = trimmed.chars().take(32).collect();
    capped.trim_end_matches(['-', '.']).to_string()
}

#[cfg(test)]
#[path = "project_id.tests.rs"]
mod tests;
