use super::FileAction;
use super::manifest::ScaffoldManifest;

/// How a scaffold file should be handled during re-init (when files already exist).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdatePolicy {
    /// Always overwrite — magistr internals that users should not edit.
    Overwrite,
    /// Skip if user has customized the file (detected via manifest hash comparison).
    /// Creates the file if it doesn't exist; updates only if the user hasn't changed it.
    PreserveCustomized,
    /// Never overwrite if the file exists — user-owned project files.
    NeverOverwrite,
}

impl UpdatePolicy {
    /// Decide ownership without touching disk, for both preview and generation.
    pub(crate) fn action(
        self,
        path: &str,
        existing: Option<&str>,
        generated: &str,
        manifest: &ScaffoldManifest,
    ) -> FileAction {
        let Some(existing) = existing else {
            return FileAction::Create;
        };
        if self == Self::NeverOverwrite {
            return FileAction::Preserved;
        }
        if existing == generated {
            return FileAction::Unchanged;
        }
        if self == Self::PreserveCustomized && manifest.is_customized(path, existing) {
            return FileAction::Preserved;
        }
        FileAction::Overwrite
    }
}
