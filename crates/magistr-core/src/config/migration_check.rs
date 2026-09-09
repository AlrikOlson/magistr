//! Result type for config migration checks.

use std::path::PathBuf;

use magistr_engine::tas::RoadmapBackend;

/// An auto-fixable migration action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationAction {
    /// Rename `.jig/` → `.magistr/`.
    RenameJigDir,
    /// Write the resolved `roadmap_backend` into a `project.json` that
    /// predates the key (yaml for a project with a YAML roadmap, the
    /// think-and-ship default otherwise).
    WriteRoadmapBackend(RoadmapBackend),
    /// Migrate `ROADMAP.md` → `.magistr-roadmap.yml`.
    MigrateRoadmapMd,
    /// Migrate `.magistr/roadmap.yaml` → `.magistr-roadmap.yml`.
    MigrateRoadmapYaml,
    /// Remove `jig_version` key from `.claude/settings.json`.
    RemoveJigVersionKey,
}

impl MigrationAction {
    /// Human-readable description of the action.
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::RenameJigDir => "Rename .jig/ → .magistr/".to_owned(),
            Self::WriteRoadmapBackend(backend) => format!(
                "Write \"roadmap_backend\": \"{}\" into .magistr/project.json",
                backend.as_str()
            ),
            Self::MigrateRoadmapMd => "Migrate ROADMAP.md → .magistr-roadmap.yml".to_owned(),
            Self::MigrateRoadmapYaml => {
                "Migrate .magistr/roadmap.yaml → .magistr-roadmap.yml".to_owned()
            }
            Self::RemoveJigVersionKey => {
                "Remove legacy 'jig_version' from .claude/settings.json".to_owned()
            }
        }
    }
}

/// Result of checking for migration needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationCheck {
    /// Auto-fixable migration actions.
    pub actions: Vec<MigrationAction>,
    /// Issues that require `magistr init` (not auto-fixable).
    pub manual_reasons: Vec<String>,
    /// Path to the `.claude/` directory.
    pub claude_dir: PathBuf,
}

impl MigrationCheck {
    /// Whether any migration (auto or manual) is needed.
    #[must_use]
    pub fn needs_migration(&self) -> bool {
        !self.actions.is_empty() || !self.manual_reasons.is_empty()
    }

    /// Human-readable summary of migration status.
    #[must_use]
    pub fn summary(&self) -> String {
        if !self.needs_migration() {
            return "Configuration is up to date.".to_string();
        }
        let mut lines = Vec::new();

        if !self.actions.is_empty() {
            lines.push("Auto-fixable:".to_string());
            for action in &self.actions {
                lines.push(format!("  - {}", action.description()));
            }
        }

        if !self.manual_reasons.is_empty() {
            if !self.actions.is_empty() {
                lines.push(String::new());
            }
            lines.push("Requires `magistr init`:".to_string());
            for reason in &self.manual_reasons {
                lines.push(format!("  - {reason}"));
            }
        }

        if !self.actions.is_empty() {
            lines.push(String::new());
            lines.push("Run `magistr migrate` to auto-fix.".to_string());
        }

        lines.join("\n")
    }
}
