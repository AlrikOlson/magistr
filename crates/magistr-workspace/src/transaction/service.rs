use super::{
    FormatterOptions, NativeFormatter, PreparedTransaction, TransactionError, TransactionOutcome,
    TransactionRequest, native_files::NativeFiles,
};
use crate::{
    confirmation::ConfirmationSeal, journal::JournalDb, source_languages::SourceLanguages,
    worker::WorkerSupervisor, workspace_policy::WorkspacePolicy,
};

/// Native transaction boundary under the composing host's exclusive local
/// workspace admission. The mutable journal borrow serializes local calls;
/// the host must exclude other filesystem writers for the complete lifetime.
/// This is not the production workspace owner lock or crash recovery service.
/// Preparation admits at most 256 files, 4 MiB each and 16 MiB hydrated bytes.
/// Preparation does not publish proposed source changes. Commit and compensation
/// perform no suspension after durable intent, so dropping a future cannot interrupt them.
pub struct WorkspaceTransactions<'a> {
    pub(super) policy: &'a WorkspacePolicy,
    pub(super) worker: &'a WorkerSupervisor,
    pub(super) journal: &'a mut JournalDb,
    pub(super) seal: &'a ConfirmationSeal,
    pub(super) formatter: NativeFormatter,
    pub(super) files: NativeFiles,
    /// Which files parse-verify reaches the worker for, and as what.
    pub(super) languages: SourceLanguages,
    #[cfg(test)]
    pub(super) faults: super::faults::Faults,
}
impl<'a> WorkspaceTransactions<'a> {
    pub fn new(
        policy: &'a WorkspacePolicy,
        worker: &'a WorkerSupervisor,
        journal: &'a mut JournalDb,
        seal: &'a ConfirmationSeal,
        mut options: FormatterOptions,
    ) -> Result<Self, TransactionError> {
        // Request payloads cannot widen the pipeline's per-file byte budget.
        options.max_output_bytes = options.max_output_bytes.min(4 * 1024 * 1024);
        let root = policy.check_read(".")?;
        let formatter =
            NativeFormatter::new(root.clone(), options).map_err(TransactionError::InvalidInput)?;
        Ok(Self {
            policy,
            worker,
            journal,
            seal,
            formatter,
            files: NativeFiles { root },
            languages: SourceLanguages::bundled(),
            #[cfg(test)]
            faults: Default::default(),
        })
    }
    /// Use the composing owner's tracker for cancellation cleanup.
    pub(crate) fn with_formatter_tasks(mut self, tasks: tokio_util::task::TaskTracker) -> Self {
        self.formatter = self.formatter.with_tasks(tasks);
        self
    }
    /// Select languages through the owner's handle: retained lang-pack
    /// claims answer after the bundled table.
    #[must_use]
    pub fn with_languages(mut self, languages: SourceLanguages) -> Self {
        self.languages = languages;
        self
    }

    pub async fn prepare(
        &mut self,
        request: TransactionRequest,
    ) -> Result<PreparedTransaction, TransactionError> {
        super::prepare::prepare(self, request).await
    }
    pub async fn apply(
        &mut self,
        prepared: PreparedTransaction,
        token: &str,
    ) -> TransactionOutcome {
        super::apply::apply(self, prepared, token)
    }
}
#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
