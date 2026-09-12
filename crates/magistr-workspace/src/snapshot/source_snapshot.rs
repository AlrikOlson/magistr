use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    Consistency, SCHEME, SelectionPolicy, SnapshotError, SnapshotValidation, SourceManifest,
    WorkspaceIdentity, capture, workspace_identity::frame,
};

/// One observation of a workspace's selected source: which workspace and
/// owner generation, under which scheme and selection policy, which bytes
/// at which paths with which kinds and modes, and what those bytes may
/// claim. The policy travels inside the snapshot so a later validation
/// re-runs exactly the same selection without the producing service.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceSnapshot {
    pub scheme: String,
    pub identity: WorkspaceIdentity,
    pub policy: SelectionPolicy,
    pub policy_digest: String,
    pub manifest: SourceManifest,
    pub consistency: Consistency,
    /// Wall clock at capture, for humans and ordering only; it never enters
    /// the revision.
    pub captured_at_ms: u64,
}

impl SourceSnapshot {
    /// Observe `root` lock-free: identity (canonical root, durable
    /// generation if any) and the selected bytes.
    pub fn capture(root: &Path, policy: SelectionPolicy) -> Result<Self, SnapshotError> {
        Self::capture_admitted(WorkspaceIdentity::observe(root)?, policy)
    }

    /// Observe the workspace an owner already identifies; `identity.root`
    /// must be canonical. No journal or mutation lock is involved.
    pub fn capture_admitted(
        identity: WorkspaceIdentity,
        policy: SelectionPolicy,
    ) -> Result<Self, SnapshotError> {
        let manifest = capture::capture(&identity.root, &policy)?;
        Ok(Self::assemble(
            identity,
            policy,
            manifest,
            Consistency::Optimistic,
        ))
    }

    pub(super) fn assemble(
        identity: WorkspaceIdentity,
        policy: SelectionPolicy,
        manifest: SourceManifest,
        consistency: Consistency,
    ) -> Self {
        Self {
            scheme: SCHEME.to_owned(),
            policy_digest: policy.digest(),
            identity,
            policy,
            manifest,
            consistency,
            captured_at_ms: crate::diagnostics::now_unix_ms(),
        }
    }

    /// The content-addressed revision: scheme, workspace identity, policy
    /// digest and manifest digest, framed. Two snapshots of the same
    /// workspace with the same selected bytes share it; the wall clock and
    /// the consistency claim do not participate.
    #[must_use]
    pub fn revision(&self) -> String {
        let mut digest = Sha256::new();
        frame(&mut digest, self.scheme.as_bytes());
        frame(&mut digest, self.identity.digest().as_bytes());
        frame(&mut digest, self.policy_digest.as_bytes());
        frame(&mut digest, self.manifest.digest().as_bytes());
        format!("{:x}", digest.finalize())
    }

    /// Re-observe `root` under this snapshot's policy and report what
    /// differs. A different canonical root — another checkout, another
    /// worktree — is refused: it cannot stand in for this workspace, however
    /// similar its bytes.
    pub fn validate(&self, root: &Path) -> Result<SnapshotValidation, SnapshotError> {
        self.validate_against(&WorkspaceIdentity::observe(root)?)
    }

    /// [`Self::validate`] for an identity an owner already holds.
    pub fn validate_against(
        &self,
        identity: &WorkspaceIdentity,
    ) -> Result<SnapshotValidation, SnapshotError> {
        if !self.identity.same_workspace(identity) {
            return Err(SnapshotError::ForeignWorkspace {
                expected: self.identity.root.clone(),
                actual: identity.root.clone(),
            });
        }
        let observed = capture::capture(&identity.root, &self.policy)?;
        let changes = self.manifest.changes_to(&observed);
        Ok(if changes.is_empty() {
            SnapshotValidation::Unchanged
        } else {
            SnapshotValidation::Changed(changes)
        })
    }

    /// Whether a consumer may claim it executed against these exact bytes.
    /// Only an isolated export may; an optimistic snapshot is refused even
    /// after an unchanged validation, because two agreeing observations of
    /// a shared tree say nothing about the moments between them.
    pub fn certify_immutable(&self) -> Result<(), SnapshotError> {
        match self.consistency {
            Consistency::Immutable => Ok(()),
            Consistency::Optimistic => Err(SnapshotError::UnsupportedCertification(
                "an optimistic observation cannot certify immutable inputs; matching digests before and after a run prove agreement of two reads, not isolation — execute inside a SourceExport for that claim".into(),
            )),
        }
    }
}

#[cfg(test)]
#[path = "source_snapshot.tests.rs"]
mod tests;
