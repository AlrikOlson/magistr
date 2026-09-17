use super::UpdatePolicy;
use super::manifest::ScaffoldManifest;

/// Who owns the bytes at a path, judged from the file's update policy and
/// the scaffold manifest. The judgement decides the plan's intent and is
/// shown beside it, so a reviewer can see why a file is or is not touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// Nothing is there yet; a write claims the path for the scaffold.
    Unclaimed,
    /// Magistr internals that users should not edit; every setup rewrites them.
    Internal,
    /// Written by magistr and unchanged since (the manifest hash matches).
    Generated,
    /// Written by magistr and edited since (the manifest hash differs).
    Customized,
    /// The user's file: never overwritten, or present without a manifest record.
    User,
    /// A shared file magistr merges its own entries into, keeping the rest.
    Shared,
    /// The target repository is this file's canonical home; its own bytes win.
    Canonical,
}

impl Ownership {
    /// Judge a regular scaffold file from its policy, current content and
    /// the manifest, without touching disk.
    #[must_use]
    pub(crate) fn judge(
        policy: UpdatePolicy,
        path: &str,
        existing: Option<&str>,
        manifest: &ScaffoldManifest,
    ) -> Self {
        let Some(existing) = existing else {
            return Self::Unclaimed;
        };
        match policy {
            UpdatePolicy::NeverOverwrite => Self::User,
            UpdatePolicy::Overwrite => Self::Internal,
            UpdatePolicy::PreserveCustomized => match manifest.files.get(path) {
                None => Self::User,
                Some(recorded) if *recorded == super::manifest::content_hash(existing) => {
                    Self::Generated
                }
                Some(_) => Self::Customized,
            },
        }
    }

    /// The lower-case label machine-readable output uses.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unclaimed => "unclaimed",
            Self::Internal => "internal",
            Self::Generated => "generated",
            Self::Customized => "customized",
            Self::User => "user",
            Self::Shared => "shared",
            Self::Canonical => "canonical",
        }
    }
}

#[cfg(test)]
#[path = "ownership.tests.rs"]
mod tests;
