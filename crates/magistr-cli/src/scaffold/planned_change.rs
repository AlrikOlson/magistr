use super::{Expected, FileAction, Intent, Ownership};

/// One path in an administrative change plan: what is expected there, who
/// owns it, and what the plan does about it.
#[derive(Debug, Clone)]
pub struct PlannedChange {
    /// Path relative to project root.
    pub path: String,
    /// What would happen to this file.
    pub action: FileAction,
    /// Who owns the current bytes, as judged while planning.
    pub ownership: Ownership,
    /// The content that would be written (empty for a removal).
    pub new_content: String,
    /// Existing content read while planning, if any. The expected state an
    /// application step revalidates against.
    pub existing_content: Option<String>,
    /// Snapshot of ownership evidence and project policy for Codex previews.
    /// Application rejects a stale snapshot before writing any selected file.
    pub ownership_context: Option<String>,
    /// Unified diff (only for Overwrite actions).
    pub diff: Option<String>,
}

impl PlannedChange {
    /// A change judged from its existing and generated content: create,
    /// overwrite or unchanged, with a diff for an overwrite.
    pub(crate) fn merged(path: &str, existing: Option<String>, new: String) -> Self {
        let action = match existing.as_deref() {
            None => FileAction::Create,
            Some(current) if current == new => FileAction::Unchanged,
            Some(_) => FileAction::Overwrite,
        };
        let diff = if action == FileAction::Overwrite {
            existing
                .as_deref()
                .map(|old| super::unified_diff(path, old, &new))
        } else {
            None
        };
        Self {
            path: path.to_owned(),
            action,
            ownership: Ownership::Shared,
            new_content: new,
            existing_content: existing,
            ownership_context: None,
            diff,
        }
    }

    /// The same change judged to belong to `ownership`.
    #[must_use]
    pub(crate) fn owned_by(mut self, ownership: Ownership) -> Self {
        self.ownership = ownership;
        self
    }

    /// A file its owner keeps as it is.
    pub(crate) fn preserved(path: &str, existing: Option<String>, ownership: Ownership) -> Self {
        Self {
            path: path.to_owned(),
            action: FileAction::Preserved,
            ownership,
            new_content: existing.clone().unwrap_or_default(),
            existing_content: existing,
            ownership_context: None,
            diff: None,
        }
    }

    /// A removal of `existing` at `path`.
    pub(crate) fn removal(path: &str, existing: String, ownership: Ownership) -> Self {
        Self {
            path: path.to_owned(),
            action: FileAction::Remove,
            ownership,
            new_content: String::new(),
            existing_content: Some(existing),
            ownership_context: None,
            diff: None,
        }
    }

    /// A change the plan refuses to make; the path keeps whatever is there.
    pub(crate) fn refused(
        path: &str,
        refusal: super::Refusal,
        existing: Option<String>,
        ownership: Ownership,
    ) -> Self {
        Self {
            path: path.to_owned(),
            action: FileAction::Refused(refusal),
            ownership,
            new_content: existing.clone().unwrap_or_default(),
            existing_content: existing,
            ownership_context: None,
            diff: None,
        }
    }

    /// The state the plan expects at the path when applied.
    #[must_use]
    pub fn expected(&self) -> Expected {
        Expected::of(self.existing_content.as_deref())
    }

    /// Whether the change is applied, skipped or refused.
    #[must_use]
    pub fn intent(&self) -> Intent {
        self.action.intent()
    }

    /// The reason a skipped or refused change is not applied, for a reviewer.
    #[must_use]
    pub fn reason(&self) -> Option<String> {
        match &self.action {
            FileAction::Create | FileAction::Overwrite | FileAction::Remove => None,
            FileAction::Unchanged => Some("already up to date".to_owned()),
            FileAction::Preserved => Some(
                match (self.ownership, self.existing_content.is_some()) {
                    (_, false) => "removed by the user; not recreated",
                    (Ownership::Customized, true) => "customized after magistr wrote it",
                    (Ownership::User, true) => "user-owned file",
                    (Ownership::Canonical, true) => "canonical file in its home repository",
                    _ => "kept by its owner",
                }
                .to_owned(),
            ),
            FileAction::Refused(refusal) => Some(refusal.to_string()),
        }
    }
}

#[cfg(test)]
#[path = "planned_change.tests.rs"]
mod tests;
