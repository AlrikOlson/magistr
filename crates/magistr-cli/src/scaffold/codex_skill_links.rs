use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::error::{MagistrError, Result};

/// Upgrade the old Codex view of this project's Claude skills to native files.
/// Only exact same-project, same-name workflow links qualify for migration.
#[derive(Default)]
pub(super) struct CodexSkillLinks {
    links: BTreeMap<String, PathBuf>,
}

impl CodexSkillLinks {
    pub(super) fn capture(dir: &Path) -> Result<Self> {
        let root = std::path::absolute(dir).map_err(|error| MagistrError::io(dir, error))?;
        for relative in [".agents", ".agents/skills"] {
            let path = root.join(relative);
            match std::fs::symlink_metadata(&path) {
                Ok(meta) if meta.is_dir() => {}
                Ok(_) => return Err(invalid(&path, "must be a real directory")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Self::default());
                }
                Err(error) => return Err(MagistrError::io(&path, error)),
            }
        }
        let mut links = BTreeMap::new();
        for file in super::workflow_skills::WorkflowSkills::codex_files() {
            let Some(prefix) = file.path.strip_suffix("/SKILL.md") else {
                continue;
            };
            let path = root.join(prefix);
            let target = match std::fs::read_link(&path) {
                Ok(target) => target,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(MagistrError::io(&path, error)),
            };
            let claude = prefix.replacen(".agents/", ".claude/", 1);
            let expected = root.join(&claude);
            let relative = Path::new("../..").join(&claude);
            if target != relative && target != expected {
                return Err(invalid(
                    &path,
                    "is not a legacy link to this project's matching Claude skill",
                ));
            }
            links.insert(prefix.to_owned(), target);
        }
        Ok(Self { links })
    }

    pub(super) fn covers(&self, path: &str) -> bool {
        self.prefix(path).is_some()
    }

    fn prefix(&self, path: &str) -> Option<&str> {
        self.links
            .keys()
            .find(|prefix| {
                path.strip_prefix(prefix.as_str())
                    .is_some_and(|tail| tail.starts_with('/'))
            })
            .map(String::as_str)
    }

    /// Existing text for a preview. A dangling legacy link still exists as an
    /// upgrade input, represented by empty text rather than an absent target.
    pub(super) fn read(&self, dir: &Path, path: &str) -> Result<Option<String>> {
        if self.covers(path) {
            let source = path.replacen(".agents/", ".claude/", 1);
            return Ok(Some(
                super::CodexSetup::read(dir, &source)?.unwrap_or_default(),
            ));
        }
        super::CodexSetup::read(dir, path)
    }

    pub(super) fn description(&self, path: &str) -> Option<String> {
        let prefix = self.prefix(path)?;
        Some(format!(
            "Replace legacy skill symlink {} -> {} with generated Codex files.\n",
            prefix,
            self.links.get(prefix)?.display()
        ))
    }

    pub(super) fn fingerprint(&self) -> &BTreeMap<String, PathBuf> {
        &self.links
    }

    /// A directory link and its generated companions form one migration unit.
    pub(super) fn validate_selection(&self, paths: &[&str]) -> Result<()> {
        let selected: BTreeSet<_> = paths.iter().copied().collect();
        for prefix in self.links.keys().filter(|prefix| {
            paths
                .iter()
                .any(|path| path.starts_with(&format!("{prefix}/")))
        }) {
            let required: Vec<_> = super::workflow_skills::WorkflowSkills::codex_files()
                .into_iter()
                .filter(|file| file.path.starts_with(&format!("{prefix}/")))
                .map(|file| file.path)
                .collect();
            if required.iter().any(|path| !selected.contains(path)) {
                return Err(MagistrError::Config(format!(
                    "Migrating {prefix} requires selecting its generated skill and companions together: {}; no selected files were written",
                    required.join(", ")
                )));
            }
        }
        Ok(())
    }

    /// Detach only the observed links, before any generated-file writes.
    /// The original Claude directories are never removed or written through.
    pub(super) fn migrate(&self, dir: &Path, paths: &[&str]) -> Result<()> {
        self.validate_selection(paths)?;
        let selected: Vec<_> = self
            .links
            .iter()
            .filter(|(prefix, _)| {
                paths
                    .iter()
                    .any(|path| path.starts_with(&format!("{prefix}/")))
            })
            .collect();
        for (prefix, expected) in &selected {
            let path = dir.join(prefix);
            let current =
                std::fs::read_link(&path).map_err(|error| MagistrError::io(&path, error))?;
            if &current != *expected {
                return Err(invalid(
                    &path,
                    "changed since preview; regenerate the preview",
                ));
            }
        }
        for (prefix, _) in selected {
            let path = dir.join(prefix);
            #[cfg(not(windows))]
            std::fs::remove_file(&path).map_err(|error| MagistrError::io(&path, error))?;
            #[cfg(windows)]
            std::fs::remove_dir(&path).map_err(|error| MagistrError::io(&path, error))?;
            std::fs::create_dir(&path).map_err(|error| MagistrError::io(&path, error))?;
        }
        Ok(())
    }
}

fn invalid(path: &Path, reason: &str) -> MagistrError {
    MagistrError::Config(format!("{} {reason}; left untouched", path.display()))
}

#[cfg(all(test, unix))]
#[path = "codex_skill_links.tests.rs"]
mod tests;
