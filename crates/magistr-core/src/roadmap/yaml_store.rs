//! YAML persistence layer for the roadmap.
//!
//! `YamlStore` owns a [`RoadmapIndex`] and its file path.
//! Load/save with validation and atomic writes.
//!
//! The canonical file is `.magistr-roadmap.yml` at the repo root.
//! No markdown is auto-generated — `save()` writes YAML only.

use std::path::{Path, PathBuf};

use crate::error::{MagistrError, Result};

use super::index::RoadmapIndex;
use super::schema::Roadmap;

/// YAML-backed roadmap store.
///
/// `.magistr-roadmap.yml` is the canonical source of truth.
/// All mutations go through `index_mut()`, and `save()` persists them.
#[derive(Debug)]
pub struct YamlStore {
    /// Path to the YAML file.
    yaml_path: PathBuf,
    /// In-memory index built from the YAML.
    index: RoadmapIndex,
}

impl YamlStore {
    /// Load a roadmap from a YAML file.
    ///
    /// Reads the file, deserializes, validates, and builds the index.
    pub fn load(yaml_path: impl AsRef<Path>) -> Result<Self> {
        let yaml_path = yaml_path.as_ref().to_path_buf();
        let content = std::fs::read_to_string(&yaml_path).map_err(|e| {
            MagistrError::Config(format!("failed to read {}: {e}", yaml_path.display()))
        })?;
        let roadmap: Roadmap = serde_yml::from_str(&content).map_err(|e| {
            MagistrError::Config(format!("invalid YAML in {}: {e}", yaml_path.display()))
        })?;
        roadmap.validate()?;
        let index = RoadmapIndex::new(roadmap);
        Ok(Self { yaml_path, index })
    }

    /// Create a store from an in-memory roadmap (for migration/creation).
    pub fn from_roadmap(yaml_path: impl AsRef<Path>, roadmap: Roadmap) -> Result<Self> {
        roadmap.validate()?;
        let index = RoadmapIndex::new(roadmap);
        let mut store = Self {
            yaml_path: yaml_path.as_ref().to_path_buf(),
            index,
        };
        store.index.mark_clean();
        Ok(store)
    }

    /// Load existing YAML, or create an empty roadmap if none exists.
    ///
    /// Migration order:
    /// 1. Try `.magistr-roadmap.yml` (current canonical path)
    /// 2. Try `.magistr/roadmap.yaml` (legacy path) → migrate to new path
    /// 3. Try `ROADMAP.md` (legacy markdown) → parse and migrate
    /// 4. Create an empty roadmap with the given title
    pub fn load_or_create(yaml_path: impl AsRef<Path>, title: &str) -> Result<Self> {
        let yaml_path = yaml_path.as_ref();

        // 1. Current path exists — load it
        if yaml_path.exists() {
            return Self::load(yaml_path);
        }

        // Derive project root from the yaml path
        // .magistr-roadmap.yml is at the project root, so parent is the root
        let project_root = yaml_path.parent().unwrap_or(Path::new("."));

        // 2. Legacy .magistr/roadmap.yaml — migrate to new path
        let legacy_path = super::yaml_path::legacy_yaml_path(project_root);
        if legacy_path.exists() {
            let store = Self::load(&legacy_path)?;
            let mut migrated = Self {
                yaml_path: yaml_path.to_path_buf(),
                index: store.index,
            };
            migrated.save()?;
            return Ok(migrated);
        }

        // 3. Legacy ROADMAP.md — parse and migrate
        let md_path = project_root.join("ROADMAP.md");
        if md_path.exists() {
            let roadmap = super::parse_markdown_file::parse_markdown_file(&md_path)?;
            let mut store = Self::from_roadmap(yaml_path, roadmap)?;
            store.save()?;
            return Ok(store);
        }

        // 4. Nothing exists — create empty roadmap
        let roadmap = Roadmap {
            title: title.to_string(),
            preamble: String::new(),
            vision: String::new(),
            principles: vec![],
            phases: vec![],
            epilogue: String::new(),
        };
        let mut store = Self::from_roadmap(yaml_path, roadmap)?;
        store.save()?;
        Ok(store)
    }

    /// Load from YAML, or auto-migrate from ROADMAP.md if YAML doesn't exist.
    ///
    /// This is the preferred entry point for code that should transparently
    /// handle legacy (pre-YAML) projects. If the YAML file exists, it loads
    /// normally. If not, it looks for a `ROADMAP.md` in the project root
    /// and migrates it.
    pub fn load_or_migrate(yaml_path: impl AsRef<Path>) -> Result<Self> {
        let yaml_path = yaml_path.as_ref();
        if yaml_path.exists() {
            return Self::load(yaml_path);
        }

        // Derive project root from the yaml path
        let project_root = yaml_path.parent().unwrap_or(Path::new("."));

        // Check legacy .magistr/roadmap.yaml path
        let legacy_path = super::yaml_path::legacy_yaml_path(project_root);
        if legacy_path.exists() {
            let store = Self::load(&legacy_path)?;
            let mut migrated = Self {
                yaml_path: yaml_path.to_path_buf(),
                index: store.index,
            };
            migrated.save()?;
            return Ok(migrated);
        }

        let md_path = project_root.join("ROADMAP.md");
        if !md_path.exists() {
            return Err(MagistrError::Config(format!(
                "no roadmap found: {} does not exist and no ROADMAP.md to migrate from",
                yaml_path.display()
            )));
        }

        let roadmap = super::parse_markdown_file::parse_markdown_file(&md_path)?;
        let mut store = Self::from_roadmap(yaml_path, roadmap)?;
        store.save()?;
        Ok(store)
    }

    /// Path to the YAML file.
    #[must_use]
    pub fn yaml_path(&self) -> &Path {
        &self.yaml_path
    }

    /// Immutable access to the index (for queries).
    #[must_use]
    pub fn index(&self) -> &RoadmapIndex {
        &self.index
    }

    /// Mutable access to the index (for mutations).
    pub fn index_mut(&mut self) -> &mut RoadmapIndex {
        &mut self.index
    }

    /// Save the YAML file.
    ///
    /// Writes `.magistr-roadmap.yml` atomically. Clears the dirty flag on success.
    /// No markdown is generated — use [`export_markdown`] for opt-in export.
    pub fn save(&mut self) -> Result<()> {
        let yaml = serde_yml::to_string(self.index.roadmap())
            .map_err(|e| MagistrError::Config(format!("failed to serialize roadmap: {e}")))?;

        atomic_write(&self.yaml_path, &yaml)?;
        self.index.mark_clean();
        Ok(())
    }

    /// Export roadmap as markdown to a file (opt-in, not automatic).
    ///
    /// Writes `ROADMAP.md` in the project root (parent of `.magistr-roadmap.yml`).
    pub fn export_markdown(&self) -> Result<PathBuf> {
        let project_root = self.yaml_path.parent().unwrap_or(Path::new("."));
        let policy_root = if project_root
            .file_name()
            .is_some_and(|name| name == ".magistr")
        {
            project_root.parent().unwrap_or(project_root)
        } else {
            project_root
        };
        super::ensure_file_export_allowed(policy_root)?;
        let md_path = project_root.join("ROADMAP.md");
        let content = super::markdown_gen::render(self.index.roadmap());
        super::ensure_file_export_allowed(policy_root)?;
        atomic_write(&md_path, &content)?;
        Ok(md_path)
    }

    /// Reload from disk (e.g., after external edit detected by watcher).
    pub fn reload(&mut self) -> Result<()> {
        let content = std::fs::read_to_string(&self.yaml_path).map_err(|e| {
            MagistrError::Config(format!("failed to read {}: {e}", self.yaml_path.display()))
        })?;
        let roadmap: Roadmap = serde_yml::from_str(&content).map_err(|e| {
            MagistrError::Config(format!("invalid YAML in {}: {e}", self.yaml_path.display()))
        })?;
        roadmap.validate()?;
        self.index = RoadmapIndex::new(roadmap);
        Ok(())
    }
}

/// Write content to a file atomically (write .tmp + rename).
pub(super) fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let mut tmp_path = path.as_os_str().to_os_string();
    tmp_path.push(".tmp");
    let tmp_path = PathBuf::from(tmp_path);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            MagistrError::Config(format!(
                "failed to create directory {}: {e}",
                parent.display()
            ))
        })?;
    }

    // Write to temp file
    std::fs::write(&tmp_path, content).map_err(|e| {
        MagistrError::Config(format!("failed to write {}: {e}", tmp_path.display()))
    })?;

    // Atomic rename
    std::fs::rename(&tmp_path, path).map_err(|e| {
        // Clean up temp file on failure
        let _ = std::fs::remove_file(&tmp_path);
        MagistrError::Config(format!(
            "failed to rename {} to {}: {e}",
            tmp_path.display(),
            path.display()
        ))
    })?;

    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "yaml_store.tests.rs"]
mod tests;
