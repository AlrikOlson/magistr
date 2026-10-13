//! Centralized path definitions for all Magistr-owned files.
//!
//! Single source of truth for every path that Magistr manages. Changing where
//! Magistr stores its state is a one-constant edit here — no grep required.
//!
//! ## Ownership model
//!
//! - **`.magistr/`** — Magistr-owned state (config, plans, memory, session state)
//! - **`.claude/`** — Claude Code native config (settings, rules, skills)
//!
use std::path::{Path, PathBuf};

// ── Directory names ─────────────────────────────────────────────────────────

/// Magistr's own state directory. All Magistr-managed files live here.
pub const MAGISTR_DIR: &str = ".magistr";

/// Claude Code's config directory. Rules, settings, skills live here.
pub const CLAUDE_DIR: &str = ".claude";

// ── Magistr-owned filenames ─────────────────────────────────────────────────

pub const PROJECT_JSON: &str = "project.json";
pub const PLAN_FILE: &str = "current-plan.md";
pub const TASK_FILE: &str = "current-task.md";
pub const ITERATION_LOG: &str = "iteration-log.md";
pub const MEMORY_FILE: &str = "memory.md";
pub const SESSION_STATE: &str = "session-state.json";
pub const SCAFFOLD_MANIFEST: &str = "scaffold-manifest.json";
pub const INTERVIEW_TRACE: &str = "interview-trace.log";
pub const PLUGINS_DIR_NAME: &str = "plugins";

/// Roadmap YAML filename (lives at repo root, not inside `.magistr/`).
pub const ROADMAP_YAML: &str = ".magistr-roadmap.yml";

// ── Claude-owned subdirectories ─────────────────────────────────────────────

pub const RULES_DIR: &str = "rules";
pub const SKILLS_DIR: &str = "skills";

// ── Path constructors (Magistr state → .magistr/) ──────────────────────────

/// `.magistr/` directory.
pub fn magistr_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(MAGISTR_DIR)
}

/// The project's shared `.magistr/project.json`, including from a linked working copy.
/// Invalid or unavailable Git links refuse resolution rather than selecting another file.
pub fn project_json(project_dir: &Path) -> crate::error::Result<PathBuf> {
    let location = magistr_engine::native::ProjectLocation::resolve(project_dir)
        .map_err(|error| crate::error::MagistrError::io(project_dir, error))?;
    Ok(magistr_dir(&location.authority_root).join(PROJECT_JSON))
}

/// `.magistr/current-plan.md` — active iteration plan.
pub fn plan(project_dir: &Path) -> PathBuf {
    magistr_dir(project_dir).join(PLAN_FILE)
}

/// `.magistr/current-task.md` — active task name.
pub fn task(project_dir: &Path) -> PathBuf {
    magistr_dir(project_dir).join(TASK_FILE)
}

/// `.magistr/iteration-log.md` — cumulative iteration summaries.
pub fn iteration_log(project_dir: &Path) -> PathBuf {
    magistr_dir(project_dir).join(ITERATION_LOG)
}

/// `.magistr/memory.md` — persistent project memory.
pub fn memory(project_dir: &Path) -> PathBuf {
    magistr_dir(project_dir).join(MEMORY_FILE)
}

/// `.magistr/session-state.json` — TUI session persistence.
pub fn session_state(project_dir: &Path) -> PathBuf {
    magistr_dir(project_dir).join(SESSION_STATE)
}

/// `.magistr/scaffold-manifest.json` — tracks content hashes of scaffolded files.
pub fn scaffold_manifest(project_dir: &Path) -> PathBuf {
    magistr_dir(project_dir).join(SCAFFOLD_MANIFEST)
}

/// `.magistr/interview-trace.log` — interview session trace log.
pub fn interview_trace(project_dir: &Path) -> PathBuf {
    magistr_dir(project_dir).join(INTERVIEW_TRACE)
}

/// `.magistr/plugins/` — project-local plugin directory.
pub fn plugins_dir(project_dir: &Path) -> PathBuf {
    magistr_dir(project_dir).join(PLUGINS_DIR_NAME)
}

/// `.magistr-roadmap.yml` — canonical roadmap YAML at repo root.
pub fn roadmap_yaml(project_dir: &Path) -> PathBuf {
    project_dir.join(ROADMAP_YAML)
}

// ── Path constructors (Claude Code native → .claude/) ───────────────────────

/// `.claude/` directory.
pub fn claude_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(CLAUDE_DIR)
}

/// `.claude/rules/` directory.
pub fn rules_dir(project_dir: &Path) -> PathBuf {
    claude_dir(project_dir).join(RULES_DIR)
}

/// `.claude/settings.json`.
pub fn settings_json(project_dir: &Path) -> PathBuf {
    claude_dir(project_dir).join("settings.json")
}

// ── ProjectPaths — struct wrapping a project root ───────────────────────────

/// Encapsulates all conventional paths for a Magistr project.
///
/// Holds the project root directory and provides methods for every
/// Magistr-managed path. Prefer this over the free functions when you
/// have a long-lived reference to the project directory.
#[derive(Debug, Clone)]
pub struct ProjectPaths {
    root: PathBuf,
}

impl ProjectPaths {
    /// Create a new `ProjectPaths` from a project root directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The project root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `.magistr/` directory.
    #[must_use]
    pub fn magistr_dir(&self) -> PathBuf {
        magistr_dir(&self.root)
    }

    /// The project's shared configuration; other paths stay in this working copy.
    pub fn project_json(&self) -> crate::error::Result<PathBuf> {
        project_json(&self.root)
    }

    /// `.magistr/current-plan.md` — active iteration plan.
    #[must_use]
    pub fn plan(&self) -> PathBuf {
        plan(&self.root)
    }

    /// `.magistr/current-task.md` — active task name.
    #[must_use]
    pub fn task(&self) -> PathBuf {
        task(&self.root)
    }

    /// `.magistr/iteration-log.md` — cumulative iteration summaries.
    #[must_use]
    pub fn iteration_log(&self) -> PathBuf {
        iteration_log(&self.root)
    }

    /// `.magistr/memory.md` — persistent project memory.
    #[must_use]
    pub fn memory(&self) -> PathBuf {
        memory(&self.root)
    }

    /// `.magistr/session-state.json` — TUI session persistence.
    #[must_use]
    pub fn session_state(&self) -> PathBuf {
        session_state(&self.root)
    }

    /// `.magistr/scaffold-manifest.json` — scaffold content hashes.
    #[must_use]
    pub fn scaffold_manifest(&self) -> PathBuf {
        scaffold_manifest(&self.root)
    }

    /// `.magistr/interview-trace.log` — interview session trace log.
    #[must_use]
    pub fn interview_trace(&self) -> PathBuf {
        interview_trace(&self.root)
    }

    /// `.magistr/plugins/` — project-local plugin directory.
    #[must_use]
    pub fn plugins_dir(&self) -> PathBuf {
        plugins_dir(&self.root)
    }

    /// `.magistr-roadmap.yml` — canonical roadmap YAML at repo root.
    #[must_use]
    pub fn roadmap_yaml(&self) -> PathBuf {
        roadmap_yaml(&self.root)
    }

    /// `.claude/` directory.
    #[must_use]
    pub fn claude_dir(&self) -> PathBuf {
        claude_dir(&self.root)
    }

    /// `.claude/rules/` directory.
    #[must_use]
    pub fn rules_dir(&self) -> PathBuf {
        rules_dir(&self.root)
    }

    /// `.claude/settings.json`.
    #[must_use]
    pub fn settings_json(&self) -> PathBuf {
        settings_json(&self.root)
    }

    /// Ensure the `.magistr/` directory exists.
    ///
    /// Creates the directory (and parents) if it does not exist.
    /// Idempotent — safe to call multiple times.
    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.magistr_dir())
    }
}

impl From<PathBuf> for ProjectPaths {
    fn from(root: PathBuf) -> Self {
        Self::new(root)
    }
}

impl From<&Path> for ProjectPaths {
    fn from(root: &Path) -> Self {
        Self::new(root)
    }
}

impl From<&str> for ProjectPaths {
    fn from(root: &str) -> Self {
        Self::new(PathBuf::from(root))
    }
}

impl AsRef<Path> for ProjectPaths {
    fn as_ref(&self) -> &Path {
        &self.root
    }
}

// ── Next-loop state (in /tmp, not project-local) ────────────────────────────

/// Path to the next-loop state file in `/tmp`.
pub fn loop_state(state_prefix: &str) -> PathBuf {
    PathBuf::from(format!("/tmp/{state_prefix}-next-loop"))
}

#[cfg(test)]
#[path = "paths.tests.rs"]
mod tests;
