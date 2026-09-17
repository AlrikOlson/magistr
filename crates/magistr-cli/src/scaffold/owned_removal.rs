//! Conservative cleanup of recorded regular scaffold files.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use crate::error::{MagistrError, Result};

use super::manifest::ScaffoldManifest;

/// Removal of caller-selected paths whose bytes still belong to the scaffold.
///
/// Callers own the candidate list: a manifest entry alone does not make an
/// arbitrary project file removable. Paths must be relative and must not cross
/// symlinks. Like generation, this assumes the tree is not concurrently replaced;
/// the metadata/read/unlink sequence is not a filesystem transaction.
pub(crate) struct OwnedRemoval<'a> {
    dir: &'a Path,
    manifest: &'a ScaffoldManifest,
    parents: BTreeSet<PathBuf>,
}

impl<'a> OwnedRemoval<'a> {
    pub(crate) fn new(dir: &'a Path, manifest: &'a ScaffoldManifest) -> Self {
        Self {
            dir,
            manifest,
            parents: BTreeSet::new(),
        }
    }

    /// Preserve entries without positive, current ownership evidence.
    pub(crate) fn remove(&mut self, path: &str) -> Result<()> {
        if !self.manifest.files.contains_key(path) {
            return Ok(());
        }
        let Some(full) = self.regular_file(path) else {
            return Ok(());
        };
        let Ok(content) = std::fs::read_to_string(&full) else {
            return Ok(());
        };
        if self.manifest.is_customized(path, &content) {
            return Ok(());
        }

        std::fs::remove_file(&full).map_err(|e| MagistrError::io(&full, e))?;
        let mut parent = full.parent();
        while let Some(path) = parent {
            if path == self.dir {
                break;
            }
            self.parents.insert(path.to_path_buf());
            parent = path.parent();
        }
        Ok(())
    }

    /// What removing `path` would do, judged like [`Self::remove`] judges it:
    /// nothing to say for an absent path, a removal for a recorded file
    /// whose bytes are still the scaffold's, otherwise the reason the entry
    /// is kept.
    pub(crate) fn plan(&self, path: &str) -> Option<super::PlannedChange> {
        use super::{Ownership, PlannedChange, Refusal};
        if !self.manifest.files.contains_key(path) {
            return super::read_target(self.dir, path)
                .ok()
                .flatten()
                .map(|content| PlannedChange::preserved(path, Some(content), Ownership::User));
        }
        let Some(full) = self.regular_file(path) else {
            let exists = Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
                && std::fs::symlink_metadata(self.dir.join(path)).is_ok();
            return exists.then(|| {
                PlannedChange::refused(path, Refusal::NotRegularFile, None, Ownership::Generated)
            });
        };
        let content = match std::fs::read_to_string(&full) {
            Ok(content) => content,
            Err(error) => {
                return Some(PlannedChange::refused(
                    path,
                    Refusal::Unreadable {
                        cause: error.to_string(),
                    },
                    None,
                    Ownership::Generated,
                ));
            }
        };
        Some(if self.manifest.is_customized(path, &content) {
            PlannedChange::preserved(path, Some(content), Ownership::Customized)
        } else {
            PlannedChange::removal(path, content, Ownership::Generated)
        })
    }

    /// Only directories that lost one of our files are candidates for pruning.
    pub(crate) fn prune(self) {
        for parent in self.parents.iter().rev() {
            // remove_dir is empty-only; user content and symlinks survive.
            let _ = std::fs::remove_dir(parent);
        }
    }

    fn regular_file(&self, path: &str) -> Option<PathBuf> {
        let mut full = self.dir.to_path_buf();
        let mut components = Path::new(path).components().peekable();
        while let Some(component) = components.next() {
            let Component::Normal(name) = component else {
                return None;
            };
            full.push(name);
            let metadata = std::fs::symlink_metadata(&full).ok()?;
            if components.peek().is_some() {
                if !metadata.is_dir() {
                    return None;
                }
            } else if !metadata.is_file() {
                return None;
            }
        }
        (full != self.dir).then_some(full)
    }
}
