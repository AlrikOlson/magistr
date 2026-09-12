use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::SnapshotError;
use crate::service::{ServerLock, WorkspaceGeneration};

/// Which workspace an observation is of: the canonical root — two worktrees
/// of one repository are two workspaces — and the owner generation that was
/// durable when it was taken (`None` for a directory no owner ever admitted).
/// The generation names the authority incarnation, not the source: it
/// advances on every admission and never on a write.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceIdentity {
    pub root: PathBuf,
    pub generation: Option<u64>,
}

impl WorkspaceIdentity {
    /// Observe the identity of `root` without any lock: the root is
    /// canonicalized and the durable generation is read strictly (a
    /// malformed counter is refused, a missing one is `None`).
    pub fn observe(root: &Path) -> Result<Self, SnapshotError> {
        let root = root
            .canonicalize()
            .map_err(|_| SnapshotError::RootMissing(root.to_path_buf()))?;
        if !root.is_dir() {
            return Err(SnapshotError::RootMissing(root));
        }
        let lock = root.join(".workbench").join(ServerLock::FILE_NAME);
        let generation = WorkspaceGeneration::observe(&lock)
            .map_err(|error| SnapshotError::Generation(error.to_string()))?
            .map(WorkspaceGeneration::get);
        Ok(Self { root, generation })
    }

    /// The identity an admitted owner already holds: its canonical root and
    /// the generation its lock advanced.
    #[must_use]
    pub fn admitted(root: PathBuf, generation: WorkspaceGeneration) -> Self {
        Self {
            root,
            generation: Some(generation.get()),
        }
    }

    /// Whether two identities name the same workspace; the generation may
    /// differ (a new owner) without the workspace changing.
    #[must_use]
    pub fn same_workspace(&self, other: &Self) -> bool {
        self.root == other.root
    }

    /// SHA-256 over the root bytes and the generation, length-framed.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut digest = Sha256::new();
        frame(&mut digest, self.root.as_os_str().as_encoded_bytes());
        match self.generation {
            Some(generation) => frame(&mut digest, &generation.to_le_bytes()),
            None => frame(&mut digest, b""),
        }
        format!("{:x}", digest.finalize())
    }
}

/// Length-prefixed update, so concatenated fields cannot alias each other.
pub(super) fn frame(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}
