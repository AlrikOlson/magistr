use super::{AdminOperation, FileAction, Intent, PlannedChange};

/// One reviewable administrative change plan: what an operation would do
/// to every path it considers, without writing anything.
///
/// Setup, selective apply, generation, repair, presets, import, template
/// plugins and owned removal all compute one of these and execute it, so the
/// preview a reviewer sees is the plan the writer follows. Destructive reset
/// (`magistr reset`) is deliberately not planned here.
#[derive(Debug, Clone)]
pub struct ChangePlan {
    /// The operation the plan was computed for.
    pub operation: AdminOperation,
    /// Every path considered, in the order it would be handled.
    pub files: Vec<PlannedChange>,
}

impl ChangePlan {
    /// An empty plan for `operation`.
    #[must_use]
    pub fn new(operation: AdminOperation) -> Self {
        Self {
            operation,
            files: Vec::new(),
        }
    }

    fn count(&self, action: &FileAction) -> usize {
        self.files.iter().filter(|f| f.action == *action).count()
    }

    fn count_intent(&self, intent: Intent) -> usize {
        self.files.iter().filter(|f| f.intent() == intent).count()
    }

    /// Count of files that would be created.
    #[must_use]
    pub fn creates(&self) -> usize {
        self.count(&FileAction::Create)
    }

    /// Count of files that would be overwritten.
    #[must_use]
    pub fn overwrites(&self) -> usize {
        self.count(&FileAction::Overwrite)
    }

    /// Count of files that would be removed.
    #[must_use]
    pub fn removes(&self) -> usize {
        self.count(&FileAction::Remove)
    }

    /// Count of files that are unchanged.
    #[must_use]
    pub fn unchanged(&self) -> usize {
        self.count(&FileAction::Unchanged)
    }

    /// Count of files that would be preserved (not overwritten).
    #[must_use]
    pub fn preserved(&self) -> usize {
        self.count(&FileAction::Preserved)
    }

    /// Count of files the plan refuses to touch.
    #[must_use]
    pub fn refusals(&self) -> usize {
        self.count_intent(Intent::Refuse)
    }

    /// Count of files the plan applies (creates, overwrites or removes).
    #[must_use]
    pub fn applies(&self) -> usize {
        self.count_intent(Intent::Apply)
    }

    /// Count of files the plan skips (unchanged or preserved).
    #[must_use]
    pub fn skips(&self) -> usize {
        self.count_intent(Intent::Skip)
    }

    /// The changes the plan applies, in order.
    pub fn applied(&self) -> impl Iterator<Item = &PlannedChange> {
        self.files.iter().filter(|f| f.action.is_applied())
    }

    /// The planned change for `path`, if the plan considered it.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&PlannedChange> {
        self.files.iter().find(|f| f.path == path)
    }

    /// The paths the plan refuses, each with its reason.
    #[must_use]
    pub fn refused(&self) -> Vec<(&str, String)> {
        self.files
            .iter()
            .filter_map(|f| match &f.action {
                FileAction::Refused(refusal) => Some((f.path.as_str(), refusal.to_string())),
                _ => None,
            })
            .collect()
    }
}
