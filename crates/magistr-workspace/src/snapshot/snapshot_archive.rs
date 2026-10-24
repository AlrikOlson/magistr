//! Durable source observations and scope comparison share the snapshot owner.
//! Receipts keep a revision reference; this archive retains the path-level proof.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::{SnapshotError, SourceSnapshot, WorkspaceIdentity};

fn archive(root: &Path) -> PathBuf {
    root.join(".magistr/workspace/source-snapshots")
}

fn failure(path: &Path, error: impl std::fmt::Display) -> SnapshotError {
    SnapshotError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    }
}

fn selected(path: &str, root: &str) -> bool {
    let path = path.replace('\\', "/");
    let root = root.replace('\\', "/");
    let root = root.trim_end_matches('/');
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

impl SourceSnapshot {
    /// Retain the complete observation before admitting a verification command.
    /// Publication never replaces a retained revision, including across owners.
    pub fn retain(&self) -> Result<(), SnapshotError> {
        let root = &self.identity.root;
        let revision = self.revision();
        if Self::retained(root, &revision)?.is_some() {
            return Ok(());
        }
        let directory = archive(root);
        std::fs::create_dir_all(&directory).map_err(|error| failure(&directory, error))?;
        let path = directory.join(format!("{revision}.json"));
        let mut temporary = tempfile::NamedTempFile::new_in(&directory)
            .map_err(|error| failure(&directory, error))?;
        serde_json::to_writer(&mut temporary, self).map_err(|error| failure(&path, error))?;
        temporary
            .flush()
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|error| failure(&path, error))?;
        if let Err(error) = temporary.persist_noclobber(&path) {
            // Another observation of exactly these bytes may have won publication.
            if Self::retained(root, &revision)?.is_none() {
                return Err(failure(&path, error));
            }
        }
        #[cfg(unix)]
        std::fs::File::open(&directory)
            .and_then(|file| file.sync_all())
            .map_err(|error| failure(&directory, error))?;
        Ok(())
    }

    /// Missing history is not evidence. Invalid or foreign history is refused.
    pub fn retained(root: &Path, revision: &str) -> Result<Option<Self>, SnapshotError> {
        if revision.len() != 64 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(SnapshotError::InvalidInput {
                path: revision.into(),
                reason: "invalid source revision".into(),
            });
        }
        let path = archive(root).join(format!("{revision}.json"));
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(failure(&path, error)),
        };
        let snapshot: Self =
            serde_json::from_slice(&bytes).map_err(|error| failure(&path, error))?;
        if snapshot.revision() != revision || snapshot.policy_digest != snapshot.policy.digest() {
            return Err(failure(
                &path,
                "retained source evidence does not match its revision",
            ));
        }
        let identity = WorkspaceIdentity::observe(root)?;
        if !snapshot.identity.same_workspace(&identity) {
            return Err(SnapshotError::ForeignWorkspace {
                expected: identity.root,
                actual: snapshot.identity.root,
            });
        }
        Ok(Some(snapshot))
    }

    /// Prove a current selection was covered by this observation with equal bytes.
    /// Removed, added, changed, missing and mode-changed inputs all participate.
    /// Narrowing never widens the declared input contract or relaxes other policy.
    #[must_use]
    pub fn covers(&self, later: &Self) -> bool {
        if !self.identity.same_workspace(&later.identity)
            || self.scheme != later.scheme
            || self.policy.clone().with_sources(Vec::new()).digest()
                != later.policy.clone().with_sources(Vec::new()).digest()
        {
            return false;
        }
        let before = &self.policy.source_roots;
        let now = &later.policy.source_roots;
        if !before.is_empty()
            && (now.is_empty()
                || now
                    .iter()
                    .any(|root| !before.iter().any(|old| selected(root, old))))
        {
            return false;
        }
        if now.is_empty() {
            return self.manifest == later.manifest;
        }
        let expected: BTreeMap<_, _> = self
            .manifest
            .entries
            .iter()
            .filter(|(path, _)| {
                now.iter()
                    .any(|root| selected(path, root) || ignore_control(path, root))
                    || later
                        .policy
                        .explicit_inputs()
                        .any(|root| selected(path, root))
            })
            .map(|(path, value)| (path.clone(), value.clone()))
            .collect();
        expected == later.manifest.entries
    }
}

#[cfg(test)]
#[path = "snapshot_archive.tests.rs"]
mod tests;

fn ignore_control(path: &str, source: &str) -> bool {
    let path = path.replace('\\', "/");
    let source = source.replace('\\', "/");
    let (parent, file) = path.rsplit_once('/').unwrap_or(("", path.as_str()));
    matches!(file, ".gitignore" | ".ignore") && (parent.is_empty() || selected(&source, parent))
}
