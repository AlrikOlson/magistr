//! The one executor of administrative change plans.

use crate::error::MagistrError;
use std::collections::{BTreeSet, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::manifest::ScaffoldManifest;
use super::{
    AdminOperation, ApplyFailure, ApplyOutcome, ApplyReceipt, ChangeReceipt, Expected, FileAction,
    ManifestOutcome, PlannedChange,
};

/// Applies a [`ChangePlan`](super::ChangePlan) one change at a time and
/// answers an [`ApplyReceipt`].
///
/// Setup and generation, selective apply from a dry run, explicit repair and
/// owned Codex removal all execute their plan through this service, so an
/// administrative write is always an explicit act for a named
/// [`AdminOperation`] — never something an agent session's source writes can
/// do, which take the workspace's source route instead. Explicit destructive
/// reset stays outside it.
///
/// Every change the plan means to apply is revalidated against its expected
/// state immediately before the write: a create opens the file with
/// `create_new`, so a concurrent creation fails instead of being overwritten;
/// an overwrite re-reads the target, writes a temporary file beside it and
/// renames it into place, so a failed write leaves no half-written file; a
/// removal re-reads the target first. A target that changed is reported
/// failed and left as found, and the remaining changes are still applied —
/// the receipt is per change and never claims the batch was atomic. Ownership
/// records are saved after every change that needs one, and a save that
/// fails is reported in the receipt's manifest outcome.
pub struct AdminService<'a> {
    dir: &'a Path,
    manifest: ScaffoldManifest,
    manifest_dirty: bool,
    /// Paths whose whole-file ownership is recorded in the manifest after a
    /// successful write. Merged shared files are never among them.
    owned: HashSet<String>,
    removed_parents: BTreeSet<PathBuf>,
    receipt: ApplyReceipt,
}

impl<'a> AdminService<'a> {
    /// A service for one `operation` on `dir`, loading the manifest once;
    /// `owned` names the paths whose ownership the operation records.
    pub(crate) fn open(
        dir: &'a Path,
        operation: AdminOperation,
        owned: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            dir,
            manifest: ScaffoldManifest::load(dir),
            manifest_dirty: false,
            owned: owned.into_iter().map(Into::into).collect(),
            removed_parents: BTreeSet::new(),
            receipt: ApplyReceipt::new(operation),
        }
    }

    /// The manifest as the service holds it.
    pub(crate) fn manifest(&self) -> &ScaffoldManifest {
        &self.manifest
    }

    /// The manifest as the service holds it, for records the plan itself
    /// does not carry; `finish` saves it.
    pub(crate) fn manifest_mut(&mut self) -> &mut ScaffoldManifest {
        self.manifest_dirty = true;
        &mut self.manifest
    }

    /// What is at `path` now, read the way the plan read it.
    fn observe(&self, path: &str) -> Result<Expected, ApplyFailure> {
        let current = super::read_target(self.dir, path).map_err(ApplyFailure::Blocked)?;
        Ok(Expected::of(current.as_deref()))
    }

    /// Check that `change`'s target still holds the state the plan expected.
    pub(crate) fn revalidate(&self, change: &PlannedChange) -> Result<(), ApplyFailure> {
        let expected = change.expected();
        let found = self.observe(&change.path)?;
        if found == expected {
            Ok(())
        } else {
            Err(ApplyFailure::Stale { expected, found })
        }
    }

    /// Apply one planned change and record its outcome.
    pub(crate) fn execute(&mut self, change: &PlannedChange) -> &ChangeReceipt {
        let outcome = match &change.action {
            FileAction::Refused(refusal) => ApplyOutcome::Refused {
                reason: refusal.to_string(),
            },
            FileAction::Preserved => ApplyOutcome::Skipped {
                reason: change.reason().unwrap_or_default(),
            },
            FileAction::Unchanged => self.touch_unchanged(change),
            FileAction::Remove => self.remove(change),
            FileAction::Create | FileAction::Overwrite => self.write(change),
        };
        self.receipt
            .changes
            .push(ChangeReceipt::new(change, outcome));
        self.receipt
            .changes
            .last()
            .expect("a receipt was just pushed")
    }

    /// Save the manifest if anything still needs saving and answer the
    /// receipt.
    pub(crate) fn finish(mut self) -> ApplyReceipt {
        self.save_manifest();
        // Only an owned removal takes empty parents with it; setup keeps
        // whatever directories a retired file leaves behind, as it always has.
        if self.receipt.operation == AdminOperation::Removal {
            for parent in self.removed_parents.iter().rev() {
                // remove_dir is empty-only; user content and symlinks survive.
                let _ = std::fs::remove_dir(parent);
            }
        }
        self.receipt
    }

    /// An unchanged file: its execute bits restored when the path rule says
    /// so, and its ownership recorded, provided it still is what the plan saw.
    fn touch_unchanged(&mut self, change: &PlannedChange) -> ApplyOutcome {
        if let Err(failure) = self.revalidate(change) {
            return ApplyOutcome::Failed(failure);
        }
        let path = change.path.as_str();
        let restored = if super::is_executable_path(path) {
            match ensure_executable(&self.dir.join(path)) {
                Ok(restored) => restored,
                Err(error) => {
                    return ApplyOutcome::Failed(ApplyFailure::Io {
                        cause: error.to_string(),
                    });
                }
            }
        } else {
            false
        };
        self.record(change);
        if restored {
            ApplyOutcome::Applied
        } else {
            ApplyOutcome::Skipped {
                reason: change.reason().unwrap_or_default(),
            }
        }
    }

    fn remove(&mut self, change: &PlannedChange) -> ApplyOutcome {
        if let Err(failure) = self.revalidate(change) {
            return ApplyOutcome::Failed(failure);
        }
        let root = match super::target_root::target_root(self.dir, &change.path) {
            Ok(root) => root,
            Err(error) => {
                return ApplyOutcome::Failed(ApplyFailure::Io {
                    cause: error.to_string(),
                });
            }
        };
        let full = root.join(&change.path);
        if let Err(error) = std::fs::remove_file(&full) {
            return ApplyOutcome::Failed(ApplyFailure::Io {
                cause: error.to_string(),
            });
        }
        if self.manifest.files.remove(&change.path).is_some() {
            self.manifest_dirty = true;
        }
        let mut parent = full.parent();
        while let Some(path) = parent {
            if path == root {
                break;
            }
            self.removed_parents.insert(path.to_path_buf());
            parent = path.parent();
        }
        self.save_manifest();
        ApplyOutcome::Applied
    }

    fn write(&mut self, change: &PlannedChange) -> ApplyOutcome {
        let path = change.path.as_str();
        if let Err(failure) = self.revalidate(change) {
            return ApplyOutcome::Failed(failure);
        }
        let full = match super::target_root::target_root(self.dir, path) {
            Ok(root) => root.join(path),
            Err(error) => {
                return ApplyOutcome::Failed(ApplyFailure::Io {
                    cause: error.to_string(),
                });
            }
        };
        if let Err(error) = backup_upgrade(self.dir, change) {
            return ApplyOutcome::Failed(ApplyFailure::Io {
                cause: error.to_string(),
            });
        }
        if let Err(error) =
            super::project_identity_upgrade::ProjectIdentityUpgrade::precondition(self.dir, change)
        {
            return ApplyOutcome::Failed(ApplyFailure::Io {
                cause: error.to_string(),
            });
        }
        let executable = super::is_executable_path(path);
        let creating = change.action == FileAction::Create;
        let written = if creating {
            create(&full, &change.new_content, executable)
        } else {
            replace(&full, &change.new_content, executable)
        };
        match written {
            Ok(()) => {}
            Err(error) if creating && error.kind() == std::io::ErrorKind::AlreadyExists => {
                // The file appeared between the check and the create.
                return ApplyOutcome::Failed(match self.observe(path) {
                    Ok(found) => ApplyFailure::Stale {
                        expected: Expected::Absent,
                        found,
                    },
                    Err(blocked) => blocked,
                });
            }
            Err(error) => {
                return ApplyOutcome::Failed(ApplyFailure::Io {
                    cause: error.to_string(),
                });
            }
        }
        self.record(change);
        ApplyOutcome::Applied
    }

    /// Record what a written or confirmed file's ownership is now, and save.
    fn record(&mut self, change: &PlannedChange) {
        let path = change.path.as_str();
        // A removal restores the shared components to the user's values;
        // their ownership is dropped by the caller, not re-recorded here.
        if self.receipt.operation != AdminOperation::Removal && super::CodexSetup::is_shared(path) {
            match super::CodexSetup::record(&mut self.manifest, change) {
                Ok(()) => self.manifest_dirty = true,
                Err(error) => {
                    self.receipt.manifest = ManifestOutcome::Failed {
                        cause: error.to_string(),
                    };
                    return;
                }
            }
        }
        if super::CodexSetup::owned_path(path) {
            super::CodexSetup::mark_file(&mut self.manifest, path);
            self.manifest_dirty = true;
        }
        if self.owned.contains(path) {
            self.manifest.record(path, &change.new_content);
            self.manifest_dirty = true;
        }
        self.save_manifest();
    }

    fn save_manifest(&mut self) {
        if !self.manifest_dirty {
            return;
        }
        self.receipt.manifest = match self.manifest.save(self.dir) {
            Ok(()) => {
                self.manifest_dirty = false;
                ManifestOutcome::Saved
            }
            Err(error) => ManifestOutcome::Failed {
                cause: error.to_string(),
            },
        };
    }
}

/// Save exact original bytes before the administrative writer replaces a
/// recognized older document. Content addressing makes interrupted retries safe.
fn backup_upgrade(root: &Path, change: &PlannedChange) -> crate::error::Result<()> {
    let Some(before) = &change.existing_content else {
        return Ok(());
    };
    if change.path != ".magistr/project.json"
        && super::project_upgrade::ProjectUpgrade::content(&change.path, before)?.is_none()
    {
        return Ok(());
    }
    let hash = super::manifest::content_hash(before);
    let path = format!(".magistr/upgrades/configuration/{hash}/{}", change.path);
    match super::read_target(root, &path)
        .map_err(|e| MagistrError::Config(format!("{path}: {e}")))?
    {
        Some(saved) if saved == *before => return Ok(()),
        Some(_) => {
            return Err(MagistrError::Config(format!(
                "Upgrade backup {path} changed; restore it before retrying. Your settings were left unchanged."
            )));
        }
        None => {}
    }
    let full = root.join(&path);
    let parent = full.parent().expect("backup directory");
    std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
    let save = || -> std::io::Result<()> {
        use std::io::Write;
        let mut staged = tempfile::NamedTempFile::new_in(parent)?;
        staged.write_all(before.as_bytes())?;
        staged.as_file().sync_all()?;
        match staged.persist_noclobber(&full) {
            Ok(_) => {}
            Err(error)
                if error.error.kind() == std::io::ErrorKind::AlreadyExists
                    && std::fs::read(&full)? == before.as_bytes() => {}
            Err(error) => return Err(error.error),
        }
        std::fs::File::open(parent)?.sync_all()
    };
    save().map_err(|e| MagistrError::io(&full, e))
}

/// Create `full` with `content`, failing if anything is there already.
fn create(full: &Path, content: &str, executable: bool) -> std::io::Result<()> {
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(full)?;
    file.write_all(content.as_bytes())?;
    drop(file);
    if executable {
        set_executable(full)?;
    }
    Ok(())
}

/// The temporary file `replace` writes beside `full` before renaming it into
/// place. One name per process, so a leftover is attributable.
pub(crate) fn temp_path(full: &Path) -> PathBuf {
    let name = full
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    full.with_file_name(format!(".{name}.{}.magistr-tmp", std::process::id()))
}

/// Replace `full` with `content` atomically: the bytes land in a temporary
/// file in the same directory, which is renamed over the target, so a
/// failure part-way leaves the target as it was. The target's mode is kept,
/// unless the path rule makes it executable.
fn replace(full: &Path, content: &str, executable: bool) -> std::io::Result<()> {
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = temp_path(full);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let written = (|| {
        file.write_all(content.as_bytes())?;
        drop(file);
        if executable {
            set_executable(&temp)?;
        } else if let Ok(metadata) = std::fs::metadata(full) {
            std::fs::set_permissions(&temp, metadata.permissions())?;
        }
        std::fs::rename(&temp, full)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// Give a regular file the execute bits it lacks; answers whether any
/// were missing. A target that is not a regular file is refused.
fn ensure_executable(path: &Path) -> crate::error::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata =
            std::fs::symlink_metadata(path).map_err(|e| crate::error::MagistrError::io(path, e))?;
        if !metadata.is_file() {
            return Err(crate::error::MagistrError::Config(
                "launcher must be a regular file".into(),
            ));
        }
        let mut permissions = metadata.permissions();
        let mode = permissions.mode();
        if mode & 0o111 != 0o111 {
            permissions.set_mode(mode | 0o111);
            std::fs::set_permissions(path, permissions)
                .map_err(|e| crate::error::MagistrError::io(path, e))?;
            return Ok(true);
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(false)
}

fn set_executable(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
#[path = "admin_service.tests.rs"]
mod tests;
