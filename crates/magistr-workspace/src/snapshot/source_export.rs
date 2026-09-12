use std::{fs, path::Path};

use super::{
    Consistency, EntryKind, SelectionPolicy, SnapshotError, SourceSnapshot, WorkspaceIdentity,
    capture,
};

/// The selected bytes of one observation copied into a private directory
/// this process owns, then re-observed and required to match. Nothing else
/// writes there, so a consumer that executes inside [`Self::root`] ran
/// against [`Consistency::Immutable`] inputs. The export lives as long as
/// this value; dropping it removes the directory.
pub struct SourceExport {
    directory: tempfile::TempDir,
    snapshot: SourceSnapshot,
}

impl SourceExport {
    /// Observe `root` and isolate what was observed.
    pub fn isolate(root: &Path, policy: SelectionPolicy) -> Result<Self, SnapshotError> {
        Self::isolate_admitted(WorkspaceIdentity::observe(root)?, policy)
    }

    /// [`Self::isolate`] for an identity an owner already holds.
    pub fn isolate_admitted(
        identity: WorkspaceIdentity,
        policy: SelectionPolicy,
    ) -> Result<Self, SnapshotError> {
        let origin = SourceSnapshot::capture_admitted(identity, policy)?;
        let directory = tempfile::Builder::new()
            .prefix("magistr-source-export-")
            .tempdir()
            .map_err(|error| SnapshotError::Io {
                path: "export".into(),
                detail: error.to_string(),
            })?;
        let target = directory
            .path()
            .canonicalize()
            .map_err(|error| SnapshotError::Io {
                path: "export".into(),
                detail: error.to_string(),
            })?;
        for (path, entry) in &origin.manifest.entries {
            materialize(&origin.identity.root, &target, path, entry)?;
        }
        let observed = capture::capture(&target, &origin.policy)?;
        if observed != origin.manifest {
            let changes = origin.manifest.changes_to(&observed);
            return Err(SnapshotError::ExportMismatch(
                changes
                    .iter()
                    .map(|change| format!("{} {:?}", change.path, change.kind))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
        let snapshot = SourceSnapshot::assemble(
            origin.identity,
            origin.policy,
            origin.manifest,
            Consistency::Immutable,
        );
        Ok(Self {
            directory,
            snapshot,
        })
    }

    /// The private directory holding exactly the snapshot's bytes.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.directory.path()
    }

    /// The immutable snapshot these bytes are.
    #[must_use]
    pub fn snapshot(&self) -> &SourceSnapshot {
        &self.snapshot
    }
}

fn materialize(
    origin: &Path,
    target: &Path,
    path: &str,
    entry: &super::ManifestEntry,
) -> Result<(), SnapshotError> {
    let io = |error: std::io::Error| SnapshotError::Io {
        path: path.to_owned(),
        detail: error.to_string(),
    };
    let destination = target.join(path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(io)?;
    }
    let source = origin.join(path);
    match entry.kind {
        EntryKind::Missing => Ok(()),
        EntryKind::Symlink => {
            let link = fs::read_link(&source).map_err(io)?;
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(link, &destination).map_err(io)
            }
            #[cfg(not(unix))]
            {
                let _ = link;
                Err(SnapshotError::UnsupportedCertification(format!(
                    "{path}: symlinks cannot be exported on this platform"
                )))
            }
        }
        EntryKind::Text | EntryKind::Binary => {
            fs::copy(&source, &destination).map_err(io)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&destination, fs::Permissions::from_mode(entry.mode))
                    .map_err(io)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "source_export.tests.rs"]
mod tests;
