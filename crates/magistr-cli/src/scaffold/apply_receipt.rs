use super::{AdminOperation, ApplyOutcome, ChangeReceipt, ManifestOutcome};

/// What applying an administrative change plan did: one receipt per planned
/// change, in plan order, and the manifest's outcome.
///
/// Changes are applied one by one and each is revalidated on its own, so a
/// receipt can be partial: some changes applied, some failed, the rest
/// skipped or refused as planned. Nothing here claims the batch was atomic.
#[derive(Debug, Clone)]
pub struct ApplyReceipt {
    /// The operation the plan was computed for.
    pub operation: AdminOperation,
    /// Every planned change with its outcome, in the order it was handled.
    pub changes: Vec<ChangeReceipt>,
    /// What happened to the scaffold manifest.
    pub manifest: ManifestOutcome,
}

impl ApplyReceipt {
    /// An empty receipt for `operation`.
    #[must_use]
    pub fn new(operation: AdminOperation) -> Self {
        Self {
            operation,
            changes: Vec::new(),
            manifest: ManifestOutcome::Untouched,
        }
    }

    /// The receipt for `path`, if the plan considered it.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&ChangeReceipt> {
        self.changes.iter().find(|c| c.path == path)
    }

    /// The changes that landed, in order.
    pub fn applied(&self) -> impl Iterator<Item = &ChangeReceipt> {
        self.changes.iter().filter(|c| c.outcome.is_applied())
    }

    /// The paths of the changes that landed, in order.
    #[must_use]
    pub fn applied_paths(&self) -> Vec<String> {
        self.applied().map(|c| c.path.clone()).collect()
    }

    /// The changes that were meant to land and did not, in order.
    pub fn failed(&self) -> impl Iterator<Item = &ChangeReceipt> {
        self.changes.iter().filter(|c| c.outcome.is_failed())
    }

    fn count(&self, pick: impl Fn(&ApplyOutcome) -> bool) -> usize {
        self.changes.iter().filter(|c| pick(&c.outcome)).count()
    }

    /// Whether anything the plan meant to do did not happen: a failed
    /// change or a manifest that did not save.
    #[must_use]
    pub fn is_partial(&self) -> bool {
        self.failed().next().is_some() || self.manifest.is_failed()
    }

    /// One line of counts and the manifest outcome.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{}: {} applied, {} skipped, {} refused, {} failed; {}",
            self.operation.label(),
            self.count(ApplyOutcome::is_applied),
            self.count(|o| matches!(o, ApplyOutcome::Skipped { .. })),
            self.count(|o| matches!(o, ApplyOutcome::Refused { .. })),
            self.count(ApplyOutcome::is_failed),
            self.manifest
        )
    }

    /// The error a caller answers with when the receipt is partial: what
    /// failed and why, after what was applied, so nothing reads as an
    /// all-or-nothing outcome. `None` when everything the plan meant to do
    /// happened.
    #[must_use]
    pub fn partial_error(&self) -> Option<crate::error::MagistrError> {
        if !self.is_partial() {
            return None;
        }
        let mut lines = vec![format!(
            "{} did not complete ({})",
            self.operation.label(),
            self.summary()
        )];
        lines.extend(self.failed().map(|c| format!("  {}", c.describe())));
        if self.manifest.is_failed() {
            lines.push(format!("  {}", self.manifest));
        }
        let applied = self.applied_paths();
        if !applied.is_empty() {
            lines.push(format!("  applied before that: {}", applied.join(", ")));
        }
        Some(crate::error::MagistrError::Other(lines.join("\n")))
    }
}

#[cfg(test)]
#[path = "apply_receipt.tests.rs"]
mod tests;
