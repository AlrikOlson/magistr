use super::{EditInput, FilesInput, UndoInput, UndoRow, WriteReport, source_reads::SourceReads};
use crate::{
    confirmation::ConfirmationSeal,
    contracts::{WorkspaceError, WorkspaceResponse},
    index::WorkspaceIndex,
    journal::JournalDb,
    query::WorkspaceQueries,
    transaction::FormatterOptions,
    worker::WorkerSupervisor,
    workspace_policy::WorkspacePolicy,
};

/// Native edit/files/undo verbs for one explicit session under the composing
/// host's exclusive local workspace admission. Each verb resolves intent into
/// exact changes and runs them through [`WorkspaceTransactions`]; the prepared
/// plan's token is the confirmation token, so a confirmed re-invocation must
/// reproduce the identical request and plan. This is not a tool handler,
/// workspace owner or crash-recovery service.
pub struct WorkspaceCommands<'a> {
    pub(super) policy: &'a WorkspacePolicy,
    pub(super) worker: &'a WorkerSupervisor,
    pub(super) journal: &'a mut JournalDb,
    pub(super) seal: &'a ConfirmationSeal,
    pub(super) index: Option<&'a WorkspaceIndex>,
    pub(super) options: FormatterOptions,
    pub(super) session_id: String,
    pub(super) reads: SourceReads<'a>,
    pub(super) formatter_tasks: tokio_util::task::TaskTracker,
}

impl<'a> WorkspaceCommands<'a> {
    pub fn new(
        policy: &'a WorkspacePolicy,
        worker: &'a WorkerSupervisor,
        journal: &'a mut JournalDb,
        seal: &'a ConfirmationSeal,
        index: Option<&'a WorkspaceIndex>,
        options: FormatterOptions,
        session_id: impl Into<String>,
    ) -> Result<Self, WorkspaceError> {
        Ok(Self {
            policy,
            worker,
            journal,
            seal,
            index,
            options,
            session_id: session_id.into(),
            reads: SourceReads::new(policy)?,
            formatter_tasks: tokio_util::task::TaskTracker::new(),
        })
    }

    pub(crate) fn with_formatter_tasks(mut self, tasks: tokio_util::task::TaskTracker) -> Self {
        self.formatter_tasks = tasks;
        self
    }

    pub async fn edit(&mut self, input: &EditInput) -> WorkspaceResponse<WriteReport> {
        super::edit_resolve::run(self, input).await
    }

    pub async fn files(&mut self, input: &FilesInput) -> WorkspaceResponse<WriteReport> {
        super::files_resolve::run(self, input).await
    }

    pub async fn undo(&mut self, input: &UndoInput) -> WorkspaceResponse<UndoRow> {
        super::undo_run::run(self, input).await
    }

    /// Run a script in the QuickJS guest through `lane`; its queued writes
    /// resolve through the same confirm gate and pipeline as `edit`.
    #[cfg(feature = "sandbox")]
    pub async fn script(
        &mut self,
        input: &super::ScriptInput,
        interpreter: &super::ScriptInterpreter,
        lane: &dyn crate::extensions::ExtensionLane,
    ) -> WorkspaceResponse<super::ScriptRow> {
        super::script_run::run(self, input, interpreter, lane).await
    }

    pub(super) fn queries(&self) -> Result<WorkspaceQueries<'_>, WorkspaceError> {
        WorkspaceQueries::new(self.policy, self.worker, self.index)
    }
}

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
