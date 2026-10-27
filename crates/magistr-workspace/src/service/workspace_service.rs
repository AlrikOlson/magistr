use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use serde_json::Value;
use tokio::sync::Mutex;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{
    IndexBuildTask, JournalListInput, ServerLock, StatusRow, WorkspaceGeneration,
    WorkspaceOpenError, WorkspaceOpenOptions, WorkspaceSession, journal_list,
};
use crate::{
    commands::{
        EditInput, FilesInput, UndoInput, UndoRow, WorkspaceCommands, WriteInput, WriteReport,
    },
    config::ResolvedConfig,
    confirmation::ConfirmationSeal,
    contracts::{ErrorKind, Freshness, WorkspaceError, WorkspaceResponse},
    diagnostics::{JournalRow, TraceRecorder},
    execution::{ProcessExecutor, RunInput, RunRow, WorkspaceRun},
    index::{
        IndexOptions, WorkspaceIndex,
        watch::{NetworkDetection, WatchMode},
    },
    jail::WorkspaceKeyStore,
    journal::{JournalDb, JournalDbOptions, JournalMode},
    query::{
        MapInput, MapRow, ReadInput, ReadRow, SearchGroup, SearchInput, SourceObservation,
        WorkspaceQueries,
    },
    snapshot::{
        MutationReceipt, SelectionPolicy, SnapshotError, SnapshotValidation, SourceSnapshot,
        WorkspaceIdentity,
    },
    source_languages::SourceLanguages,
    transaction::FormatterOptions,
    worker::WorkerSupervisor,
    workspace_policy::WorkspacePolicy,
};

/// One admitted workspace and every typed use case over it: the policy, the
/// supervised worker, the derived index, the sole journal writer, the
/// confirmation seal, the extension lanes, the owned-process executor and
/// the flight recorder, composed once and shared by every request. Hosts
/// negotiate their wire and call these methods; the answers are typed
/// responses with this workspace's deterministic notes appended.
pub struct WorkspaceService {
    root: PathBuf,
    state_dir: PathBuf,
    policy: WorkspacePolicy,
    worker: Arc<WorkerSupervisor>,
    index: WorkspaceIndex,
    journal: Mutex<JournalDb>,
    seal: ConfirmationSeal,
    executor: ProcessExecutor,
    trace: TraceRecorder,
    formatter: FormatterOptions,
    formatter_tasks: TaskTracker,
    detection: NetworkDetection,
    applied_journal_mode: String,
    notes: Vec<String>,
    /// When the first index build was kicked: the task's start until the
    /// coordinator stamps its own.
    opened_at_ms: u64,
    /// Which files parse as what, everywhere this workspace reads source:
    /// the bundled table, then the lang-pack claims the owner retains.
    languages: SourceLanguages,
    #[cfg(feature = "sandbox")]
    lane: Arc<crate::extensions::WorkerExtensionLane>,
    #[cfg(feature = "sandbox")]
    ranking: crate::extensions::RankingSignals,
    /// The admitted lang packs and their replay into every worker
    /// incarnation; `None` when no pack is installed.
    #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
    lang_packs: Option<Arc<crate::extensions::LangPackRestore>>,
    #[cfg(feature = "sandbox")]
    resolver_specs: Vec<crate::extensions::LanguageServerSpec>,
    #[cfg(feature = "sandbox")]
    interpreter: Option<crate::commands::ScriptInterpreter>,
    lock: ServerLock,
    _lifecycle_lock: Option<std::fs::File>,
}

impl WorkspaceService {
    /// Admit and compose one workspace. The order is the retained one: the
    /// root must exist, the state directory is created, the OS-held lock is
    /// taken before any store opens, configuration resolves (a layer from a
    /// newer binary fails closed), the key exists before the first request,
    /// the journal opens in the mode the filesystem detector chose and the
    /// applied mode is read back, then the worker, the extension lanes (the
    /// admitted lang packs are installed as the worker's restore before
    /// anything can spawn it), the index and the recorder compose over the
    /// admitted policy.
    pub async fn open(options: WorkspaceOpenOptions) -> Result<Self, WorkspaceOpenError> {
        let root = options
            .root
            .canonicalize()
            .map_err(|_| WorkspaceOpenError::RootMissing(options.root.clone()))?;
        if !root.is_dir() {
            return Err(WorkspaceOpenError::RootMissing(root));
        }
        let state_dir = root.join(".magistr/workspace");
        std::fs::create_dir_all(&state_dir)?;
        let lock = ServerLock::acquire(state_dir.join(ServerLock::FILE_NAME))
            .map_err(WorkspaceOpenError::lock)?;
        let lifecycle_lock = options
            .lifecycle_lock
            .as_deref()
            .map(super::lifecycle_lock::acquire)
            .transpose()?;
        let upgrade = super::workspace_upgrade::WorkspaceUpgrade::acquire(&root)
            .map_err(WorkspaceOpenError::lock)?;
        crate::config::WorkspaceConfigUpgrade::apply(&root)?;
        let detection = NetworkDetection::detect(&root, options.network_override.as_deref());
        let policy = WorkspacePolicy::resolve(&options.resolve_options(root.clone()))
            .map_err(WorkspaceOpenError::policy)?;
        let key = WorkspaceKeyStore::new(&state_dir).ensure()?;
        let mut journal_options = JournalDbOptions::new(state_dir.join("journal.db"));
        journal_options.journal_mode = if detection.network_fs {
            JournalMode::Truncate
        } else {
            JournalMode::Wal
        };
        let journal = JournalDb::open(journal_options).map_err(WorkspaceOpenError::journal)?;
        if let Some(upgrade) = upgrade {
            upgrade.finish(&journal)?;
        }
        let applied_journal_mode = journal
            .journal_mode()
            .map_err(WorkspaceOpenError::journal)?;
        let worker = Arc::new(WorkerSupervisor::new(
            options.worker_executable.clone(),
            options.worker_options.clone(),
        )?);
        let languages = SourceLanguages::bundled();
        #[cfg(feature = "sandbox")]
        let extensions = compose_extensions(&policy, &root, &state_dir, &worker, &languages).await;
        let index = WorkspaceIndex::start(
            root.clone(),
            state_dir.clone(),
            Arc::clone(&worker),
            IndexOptions {
                watch_mode: watch_mode(policy.config()),
                network_override: options.network_override.clone(),
                languages: languages.clone(),
                ..IndexOptions::default()
            },
        )?;
        let trace = if tracing_enabled(policy.config()) {
            TraceRecorder::new(state_dir.join("trace.jsonl"))
        } else {
            TraceRecorder::disabled()
        };
        let notes = workspace_notes(&journal, &detection, policy.config());
        Ok(Self {
            executor: ProcessExecutor::new(root.clone()),
            root,
            state_dir,
            policy,
            worker,
            index,
            journal: Mutex::new(journal),
            seal: ConfirmationSeal::new(&key, lock.generation().get()),
            trace,
            formatter: options.formatter.clone(),
            formatter_tasks: TaskTracker::new(),
            detection,
            applied_journal_mode,
            notes,
            opened_at_ms: crate::diagnostics::now_unix_ms(),
            languages,
            #[cfg(feature = "sandbox")]
            lane: extensions.lane,
            #[cfg(feature = "sandbox")]
            ranking: extensions.ranking,
            #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
            lang_packs: extensions.lang_packs,
            #[cfg(feature = "sandbox")]
            resolver_specs: extensions.specs,
            #[cfg(feature = "sandbox")]
            interpreter: options.interpreter,
            lock,
            _lifecycle_lock: lifecycle_lock,
        })
    }

    /// Admit an explicit journal identity to this workspace incarnation.
    pub fn admit_session(&self, id: impl Into<String>) -> Result<WorkspaceSession, WorkspaceError> {
        WorkspaceSession::new(self.root.clone(), self.generation(), id.into())
    }

    /// Mint a distinct identity for a client that did not supply one.
    pub fn new_session(&self) -> Result<WorkspaceSession, WorkspaceError> {
        self.admit_session(super::mint_session_identity("native")?)
    }

    /// Ownership incarnation, independent of index progress and source revisions.
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.lock.generation()
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    #[must_use]
    pub fn config(&self) -> &ResolvedConfig {
        self.policy.config()
    }

    #[must_use]
    pub fn trace(&self) -> &TraceRecorder {
        &self.trace
    }

    /// Deterministic degradations every envelope this workspace answers carries.
    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// Whether a composed interpreter lets this workspace offer the script verb.
    #[must_use]
    pub fn offers_script(&self) -> bool {
        #[cfg(feature = "sandbox")]
        {
            self.interpreter.is_some()
        }
        #[cfg(not(feature = "sandbox"))]
        {
            false
        }
    }

    /// Token mode, the default, never elicits a human; only an explicit
    /// `edit.confirm_mode = "elicit"` turns elicitation on. Writes normally apply
    /// immediately; when `edit.force_confirm` restores a gate, its response
    /// carries the confirmation token instead of opening a dialog.
    #[must_use]
    pub fn elicitation_allowed(&self) -> bool {
        self.policy
            .config()
            .effective
            .get("edit.confirm_mode")
            .and_then(Value::as_str)
            != Some("token")
    }

    /// A refusal decided before any use case ran (a garbled negotiation, an
    /// absent lane), carrying this workspace's notes like every answer.
    #[must_use]
    pub fn refusal<R>(&self, error: WorkspaceError) -> WorkspaceResponse<R> {
        self.with_notes(WorkspaceResponse::Failure {
            version: crate::contracts::EnvelopeVersion::LATEST,
            error,
            notes: None,
        })
    }

    /// Observe the worker and index owned by this service; plugin state
    /// and runtime degradations ride the notes.
    pub async fn status(&self) -> WorkspaceResponse<StatusRow> {
        #[allow(unused_mut)]
        let mut notes = self.notes.clone();
        #[cfg(feature = "sandbox")]
        notes.extend(self.ranking.status_notes());
        #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
        if let Some(packs) = &self.lang_packs {
            notes.extend(packs.status_notes());
        }
        StatusRow::new(
            &self.root,
            &self.detection,
            self.applied_journal_mode.clone(),
            self.worker.status().await,
            self.index.status(),
        )
        .respond(notes)
    }

    pub async fn map(&self, input: &MapInput) -> WorkspaceResponse<MapRow> {
        match self.queries(CancellationToken::new()) {
            Ok(queries) => self.with_notes(queries.map(input).await),
            Err(error) => self.with_notes(crate::query::budget::failure(error)),
        }
    }

    pub async fn read(&self, input: &ReadInput) -> WorkspaceResponse<ReadRow> {
        match self.queries(CancellationToken::new()) {
            Ok(queries) => self.with_notes(queries.read(input).await),
            Err(error) => self.with_notes(crate::query::budget::failure(error)),
        }
    }

    /// Read one admitted file as exact bytes, bounded by the live reader's
    /// 1 MiB ceiling. Freshness describes this observation, independently of
    /// the derived index. A changed file must be selected again by the caller.
    pub fn read_source(
        &self,
        path: &str,
        expected_hash: Option<&str>,
    ) -> Result<SourceObservation, WorkspaceError> {
        let observation = self.queries(CancellationToken::new())?.snapshot(path)?;
        if observation.freshness != Freshness::Warm {
            return Err(WorkspaceError::new(
                ErrorKind::StaleAnchor {
                    content_hash: observation.content_hash,
                },
                format!(
                    "{} changed while it was being read; read it again",
                    observation.path
                ),
            ));
        }
        if expected_hash.is_some_and(|expected| expected != observation.content_hash) {
            return Err(WorkspaceError::new(
                ErrorKind::StaleAnchor {
                    content_hash: observation.content_hash,
                },
                format!(
                    "{} changed since it was selected; read it again",
                    observation.path
                ),
            ));
        }
        Ok(observation)
    }

    /// `cancel` is the request's own token: a client cancellation stops the
    /// worker call instead of letting the search burn on.
    pub async fn search(
        &self,
        input: &SearchInput,
        cancel: CancellationToken,
    ) -> WorkspaceResponse<SearchGroup> {
        let queries = match self.queries(cancel) {
            Ok(queries) => queries,
            Err(error) => return self.with_notes(crate::query::budget::failure(error)),
        };
        #[cfg(feature = "sandbox")]
        if !self.resolver_specs.is_empty() {
            let host = self.plugin_host();
            let resolver = crate::extensions::PluginResolver::new(
                &host,
                self.root.to_string_lossy(),
                self.resolver_specs.clone(),
            );
            let response = queries.with_resolver(&resolver).search(input).await;
            return self.with_notes(response);
        }
        self.with_notes(queries.search(input).await)
    }

    /// Writes serialize on the sole journal writer, as they did behind the
    /// retained write lock.
    pub async fn edit(
        &self,
        input: &EditInput,
        session: &WorkspaceSession,
    ) -> WorkspaceResponse<WriteReport> {
        let mut journal = self.journal.lock().await;
        let response = match self.commands(&mut journal, session) {
            Ok(mut commands) => commands.edit(input).await,
            Err(error) => crate::query::budget::failure(error),
        };
        self.with_notes(response)
    }

    pub async fn files(
        &self,
        input: &FilesInput,
        session: &WorkspaceSession,
    ) -> WorkspaceResponse<WriteReport> {
        let mut journal = self.journal.lock().await;
        let response = match self.commands(&mut journal, session) {
            Ok(mut commands) => commands.files(input).await,
            Err(error) => crate::query::budget::failure(error),
        };
        self.with_notes(response)
    }

    /// The whole content of one file as exact bytes through the same
    /// pipeline: Magistr's own source writes after the session's
    /// permission decision, journaled like every other mutation.
    pub async fn write(
        &self,
        input: &WriteInput,
        session: &WorkspaceSession,
    ) -> WorkspaceResponse<WriteReport> {
        let mut journal = self.journal.lock().await;
        let response = match self.commands(&mut journal, session) {
            Ok(mut commands) => commands.write(input).await,
            Err(error) => crate::query::budget::failure(error),
        };
        self.with_notes(response)
    }

    /// The rewind half of the undo tool.
    pub async fn undo(
        &self,
        input: &UndoInput,
        session: &WorkspaceSession,
    ) -> WorkspaceResponse<UndoRow> {
        let mut journal = self.journal.lock().await;
        let response = match self.commands(&mut journal, session) {
            Ok(mut commands) => commands.undo(input).await,
            Err(error) => crate::query::budget::failure(error),
        };
        self.with_notes(response)
    }

    /// The read half of the undo tool (`action: "list"`): nothing is written.
    pub async fn journal_list(
        &self,
        input: &JournalListInput,
        session: &WorkspaceSession,
    ) -> WorkspaceResponse<JournalRow> {
        if let Err(error) = session.validate(&self.root, self.generation()) {
            return self.refusal(error);
        }
        let journal = self.journal.lock().await;
        self.with_notes(journal_list::list(&journal, input, session.id()))
    }

    /// `None` when no interpreter is composed: the verb is not offered at all.
    #[cfg(feature = "sandbox")]
    pub async fn script(
        &self,
        input: &crate::commands::ScriptInput,
        session: &WorkspaceSession,
    ) -> Option<WorkspaceResponse<crate::commands::ScriptRow>> {
        if let Err(error) = session.validate(&self.root, self.generation()) {
            return Some(self.refusal(error));
        }
        let interpreter = self.interpreter.as_ref()?;
        let mut journal = self.journal.lock().await;
        let response = match self.commands(&mut journal, session) {
            Ok(mut commands) => commands.script(input, interpreter, &*self.lane).await,
            Err(error) => crate::query::budget::failure(error),
        };
        Some(self.with_notes(response))
    }

    /// One declared command; installed runner plugins are consulted before
    /// the bundled parsers.
    pub async fn run(
        &self,
        input: &RunInput,
        session: &WorkspaceSession,
        cancel: CancellationToken,
    ) -> WorkspaceResponse<RunRow> {
        if let Err(error) = session.validate(&self.root, self.generation()) {
            return self.refusal(error);
        }
        #[cfg(feature = "sandbox")]
        {
            use crate::{execution::PluginRunnerLane, extensions::RunnerAdapter};
            let host = self.plugin_host();
            let (components, _) = host.runner_components();
            let adapters: Vec<RunnerAdapter<'_>> = components
                .into_iter()
                .map(|(name, component)| RunnerAdapter::new(name, component.bytes, &*self.lane))
                .collect();
            let lanes: Vec<&dyn PluginRunnerLane> = adapters
                .iter()
                .map(|adapter| adapter as &dyn PluginRunnerLane)
                .collect();
            let response = WorkspaceRun::new(&self.policy, &self.executor)
                .with_plugin_lanes(lanes)
                .run(input, cancel)
                .await;
            self.with_notes(response)
        }
        #[cfg(not(feature = "sandbox"))]
        {
            self.with_notes(
                WorkspaceRun::new(&self.policy, &self.executor)
                    .run(input, cancel)
                    .await,
            )
        }
    }

    /// Every invocable plugin tool under its derived wire name, plus the
    /// discovery notes.
    #[cfg(feature = "sandbox")]
    #[must_use]
    pub fn plugin_tools(&self) -> (Vec<crate::extensions::PluginToolListing>, Vec<String>) {
        self.plugin_host().list_tools()
    }

    /// `None` when no installed plugin derives `wire_name`.
    #[cfg(feature = "sandbox")]
    pub async fn run_plugin_tool(
        &self,
        wire_name: &str,
        args: &Value,
        budget: u64,
        session: &WorkspaceSession,
    ) -> Option<WorkspaceResponse<Value>> {
        if let Err(error) = session.validate(&self.root, self.generation()) {
            return Some(self.refusal(error));
        }
        let host = self.plugin_host();
        let response = host.run_tool(wire_name, args, budget).await?;
        Some(self.with_notes(response))
    }

    /// The identity this owner holds: its canonical root and the generation
    /// its admission advanced.
    #[must_use]
    pub fn identity(&self) -> WorkspaceIdentity {
        WorkspaceIdentity::admitted(self.root.clone(), self.generation())
    }

    /// Observe the selected source of this workspace. The read takes no
    /// journal or mutation lock: a consumer captures, runs its subprocess,
    /// and validates afterwards while edits keep flowing through the owner.
    pub fn source_snapshot(
        &self,
        policy: SelectionPolicy,
    ) -> Result<SourceSnapshot, SnapshotError> {
        SourceSnapshot::capture_admitted(self.identity(), policy)
    }

    /// Re-observe this workspace against `snapshot`; a snapshot of another
    /// workspace is refused rather than compared.
    pub fn validate_snapshot(
        &self,
        snapshot: &SourceSnapshot,
    ) -> Result<SnapshotValidation, SnapshotError> {
        snapshot.validate_against(&self.identity())
    }

    /// The truthful receipt of journal op `seq`, read from its resolved
    /// intent; `None` when no such op exists. The journal writer is borrowed
    /// only for the read.
    pub async fn mutation_receipt(
        &self,
        seq: i64,
    ) -> Result<Option<MutationReceipt>, WorkspaceError> {
        let journal = self.journal.lock().await;
        MutationReceipt::from_journal(&journal, seq, Some(self.generation().get())).map_err(
            |error| {
                WorkspaceError::new(
                    crate::contracts::ErrorKind::EngineUnavailable,
                    error.to_string(),
                )
            },
        )
    }

    /// The one background task. The first build is kicked at open, so the
    /// task exists from open on; `None` only once the index has stopped.
    #[must_use]
    pub fn index_task(&self) -> Option<IndexBuildTask> {
        IndexBuildTask::since_open(&self.index.status(), self.opened_at_ms)
    }

    /// The derived index's own progress row — cache progress, never a
    /// source revision — for hosts that observe a build (the bench, a
    /// cockpit) without walking the tree through `map`.
    #[must_use]
    pub fn index_status(&self) -> crate::index::IndexStatus {
        self.index.status()
    }

    /// Stop the index, reap the worker and release the lock. Nothing under
    /// the state directory is removed.
    pub async fn shutdown(mut self) -> Result<(), WorkspaceError> {
        // Handler lifetimes have ended before the host consumes this service,
        // so no new formatter can be admitted. Drain owned cancellation work
        // before a fallible index/worker shutdown or release of the OS lock.
        self.formatter_tasks.close();
        self.formatter_tasks.wait().await;
        self.index.shutdown().await.map_err(|error| {
            WorkspaceError::new(
                crate::contracts::ErrorKind::EngineUnavailable,
                error.to_string(),
            )
        })?;
        self.worker
            .shutdown()
            .await
            .map_err(crate::query::worker_error)?;
        Ok(())
    }

    /// The owner's language selection: bundled first, then the lang-pack
    /// claims the last worker incarnation registered.
    #[must_use]
    pub fn languages(&self) -> &SourceLanguages {
        &self.languages
    }

    /// The supervised worker as it stands: pid of the admitted incarnation,
    /// restarts, failures, quarantine, admission — an observation for hosts
    /// and fixtures, never a handle to the process.
    pub async fn worker_status(&self) -> crate::worker::WorkerStatus {
        self.worker.status().await
    }

    /// The last worker incarnation's lang-pack rows, admission refusals
    /// first; empty without installed packs.
    #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
    #[must_use]
    pub fn lang_pack_statuses(&self) -> Vec<crate::extensions::LangPackStatus> {
        self.lang_packs
            .as_ref()
            .map(|packs| packs.statuses())
            .unwrap_or_default()
    }

    fn queries(&self, cancel: CancellationToken) -> Result<WorkspaceQueries<'_>, WorkspaceError> {
        let queries = WorkspaceQueries::new(&self.policy, &self.worker, Some(&self.index))?
            .with_state_dir(self.state_dir.clone())
            .with_cancel(cancel);
        #[cfg(feature = "sandbox")]
        let queries = queries.with_ranking(&self.ranking);
        Ok(queries)
    }

    fn commands<'a>(
        &'a self,
        journal: &'a mut JournalDb,
        session: &WorkspaceSession,
    ) -> Result<WorkspaceCommands<'a>, WorkspaceError> {
        session.validate(&self.root, self.generation())?;
        WorkspaceCommands::new(
            &self.policy,
            &self.worker,
            journal,
            &self.seal,
            Some(&self.index),
            self.formatter.clone(),
            session.id(),
        )
        .map(|commands| {
            commands
                .with_formatter_tasks(self.formatter_tasks.clone())
                .with_languages(self.languages.clone())
        })
    }

    #[cfg(feature = "sandbox")]
    fn plugin_host(&self) -> crate::extensions::PluginHost<'_> {
        crate::extensions::PluginHost::new(&self.policy, self.state_dir.clone(), &*self.lane)
    }

    /// Degradations ride every envelope's notes, errors included, after the
    /// answer's own notes.
    fn with_notes<R>(&self, mut response: WorkspaceResponse<R>) -> WorkspaceResponse<R> {
        if self.notes.is_empty() {
            return response;
        }
        let notes = match &mut response {
            WorkspaceResponse::Success { notes, .. } | WorkspaceResponse::Failure { notes, .. } => {
                notes
            }
        };
        notes
            .get_or_insert_with(Vec::new)
            .extend(self.notes.iter().cloned());
        response
    }
}

fn watch_mode(config: &ResolvedConfig) -> WatchMode {
    config
        .effective
        .get("watcher.mode")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn tracing_enabled(config: &ResolvedConfig) -> bool {
    config
        .effective
        .get("tracing.enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

/// The retained deterministic notes: journal lifecycle, the network
/// filesystem degradation and configuration warnings — no pids, no clocks.
fn workspace_notes(
    journal: &JournalDb,
    detection: &NetworkDetection,
    config: &ResolvedConfig,
) -> Vec<String> {
    let mut notes = journal.lifecycle_notes.clone();
    if detection.network_fs {
        let fstype = detection
            .fstype
            .as_ref()
            .map(|fstype| format!(": {fstype}"))
            .unwrap_or_default();
        notes.push(format!(
            "workspace is on a network filesystem (detected via {}{fstype}): SQLite journal mode degraded WAL→TRUNCATE; concurrent instances are refused by the advisory lock",
            detection.method
        ));
    }
    notes.extend(config.notes());
    notes
}

/// The production extension lanes over the supervised worker.
#[cfg(feature = "sandbox")]
struct Extensions {
    lane: Arc<crate::extensions::WorkerExtensionLane>,
    ranking: crate::extensions::RankingSignals,
    specs: Vec<crate::extensions::LanguageServerSpec>,
    #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
    lang_packs: Option<Arc<crate::extensions::LangPackRestore>>,
}

/// Ranking signals booted now; installed lang packs admitted now and
/// installed as the worker's restore before anything can spawn it, then the
/// first incarnation is forced so the retained registrations and the
/// selection table exist before the index's first extraction (a workspace
/// with no packs never spawns here); and the language-server detection
/// table the resolver lane consults per request. A worker that cannot be
/// restored at open does not refuse the open: the restore stays installed,
/// the next request spawns and restores again, and the status notes name
/// the failure until an incarnation is restored.
#[cfg(feature = "sandbox")]
async fn compose_extensions(
    policy: &WorkspacePolicy,
    root: &Path,
    state_dir: &Path,
    worker: &Arc<WorkerSupervisor>,
    languages: &SourceLanguages,
) -> Extensions {
    use crate::extensions::{ExtensionLane, RankingSignals, WorkerExtensionLane};
    let lane = Arc::new(WorkerExtensionLane::new(Arc::clone(worker)));
    let shared: Arc<dyn ExtensionLane> = lane.clone();
    let ranking = RankingSignals::new(state_dir, shared);
    ranking.boot(policy, state_dir);
    #[cfg(not(all(feature = "sandbox", feature = "lang-pack")))]
    let _ = languages;
    #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
    let lang_packs = {
        let host = crate::extensions::PluginHost::new(policy, state_dir.to_path_buf(), &*lane);
        let admission = crate::extensions::LangPackBoot::new(&host).admit();
        if admission.is_empty() {
            None
        } else {
            let restore = Arc::new(crate::extensions::LangPackRestore::new(
                admission,
                state_dir,
                languages.clone(),
            ));
            worker.install_restore(restore.clone());
            // The answer is the restore's: a failure is already retained
            // on the restore's rows and repeats on the next spawn.
            let _ = worker
                .call(
                    crate::worker::WorkerRequest::Ping,
                    std::time::Duration::from_secs(120),
                    &CancellationToken::new(),
                )
                .await;
            Some(restore)
        }
    };
    let specs = crate::extensions::LanguageServerSpec::detect(root);
    Extensions {
        lane,
        ranking,
        specs,
        #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
        lang_packs,
    }
}

#[cfg(test)]
#[path = "workspace_service.tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_snapshot.tests.rs"]
mod snapshot_tests;
