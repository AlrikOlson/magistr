//! Core reasoning-step engine. Owns the `ReasoningServer` struct and its sibling impl blocks.

use magistr_workflow_contracts::{Observed, ReadError, ReadSource};

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use chrono::Utc;

use crate::cloud::client::CloudClient;
use crate::think::broadcast::Broadcaster;
use crate::think::config::{ThinkConfig, namespace_session_id, resolve_project_id};
use crate::think::domain::{
    Branch, HistoryMetadata, NextAction, SessionEntry, ThinkHistory, ThinkStep,
};
use crate::think::formatter::Formatter;
use crate::think::persistence::Persistence;

pub struct ReasoningServer {
    // Fields are `pub(crate)` so sibling modules under `engine::*` can
    // share access without bouncing through accessors. Anything outside
    // this crate still goes through the `config()` / `history()` /
    // `branches()` / `sessions()` accessors.
    pub(crate) history: ThinkHistory,
    pub(crate) config: ThinkConfig,
    pub(crate) formatter: Formatter,
    pub(crate) persistence: Persistence,
    pub(crate) start_time: Instant,
    pub(crate) branches: HashMap<String, Branch>,
    pub(crate) sessions: HashMap<String, SessionEntry>,
    pub(crate) step_index: HashMap<u32, usize>,
    pub(crate) step_numbers: HashSet<u32>,
    pub(crate) tools_used: HashSet<String>,
    pub(crate) step_to_branch: HashMap<u32, String>,
    pub(crate) branch_depth_cache: HashMap<u32, u32>,
    pub(crate) steps_since_cleanup: u32,
    pub(crate) branches_seq: u64,
    /// When sessions are enabled, the session whose history is currently
    /// loaded into `self.history`. `None` means we're on the default
    /// non-session history.
    pub(crate) active_session: Option<String>,
    /// Optional NDJSON-over-Unix-socket fan-out. Spawned only when
    /// `config.broadcast.path` is set; absent (or failed-to-spawn) means
    /// the server runs unobserved. Calls are fire-and-forget.
    pub(crate) broadcaster: Option<Broadcaster>,
    /// Canonical working directory captured once at server start.
    /// Stamped on every recorded step's `cwd` field so the project
    /// root travels with the data — no more "where did this step come
    /// from?" archaeology when sessions span multiple projects.
    pub(crate) cwd: Option<String>,
    /// Project id resolved once at startup (`<basename>-<6hex>` or a
    /// `THINK_AND_SHIP_PROJECT_NAME` override). Used to namespace every
    /// caller-supplied session id and stamped into each session's
    /// metadata so the viewer can group without parsing.
    pub(crate) project_id: String,
    /// Optional git-native trace mirror. When set (SyncTarget::RepoGit + the
    /// process runs inside a repo), every recorded step is mirrored as an
    /// Agent Trace JSONL record into `.think-and-ship/` and the session is
    /// committed on close. All git/IO runs on the worker's own thread, never
    /// under this engine's lock. `None` = the default Local behaviour. Writes
    /// are fire-and-forget — a sink error never fails `process_step`.
    pub(crate) mirror: Option<crate::infra::MirrorWorker>,
    /// Whether mirrored records are `shared` (committed `sessions/`) vs
    /// `local` (gitignored). Default `false`. Only meaningful with `mirror`.
    pub(crate) repo_shared: bool,
    /// Optional cloud sync client. When set, every recorded step
    /// fire-and-forget pushes its envelope to the cloud backend. `None`
    /// (default) = no cloud sync.
    pub(crate) cloud: Option<CloudClient>,
}

impl ReasoningServer {
    /// Attach an externally-spawned broadcaster, overriding whatever
    /// `new()` set up from `config.broadcast.path`. Use when one
    /// process serves both tool families and they share a single
    /// underlying socket.
    pub fn with_broadcaster(mut self, broadcaster: Broadcaster) -> Self {
        self.set_broadcaster(broadcaster);
        self
    }

    /// Attach the admitted owner's publisher to an already-open engine.
    pub fn set_broadcaster(&mut self, broadcaster: Broadcaster) {
        self.broadcaster = Some(broadcaster);
    }

    /// The broadcaster this engine emits through, if one was bound.
    ///
    /// Exposed so an observer that has connected to the socket can wait
    /// for the accept loop to actually register it before driving
    /// mutations — connecting alone does not make a client a subscriber,
    /// and frames emitted before registration are dropped. See
    /// [`crate::infra::Broadcaster::subscriber_count`].
    #[must_use]
    pub fn broadcaster(&self) -> Option<&Broadcaster> {
        self.broadcaster.as_ref()
    }

    /// Attach a cloud client so every recorded step fire-and-forget pushes its
    /// envelope to the cloud backend. Wired by the host's composition root.
    pub fn with_cloud(mut self, client: CloudClient) -> Self {
        self.cloud = Some(client);
        self
    }

    /// Fire-and-forget cloud push of the most recently appended step.
    /// No-op without a cloud client OR outside a tokio runtime (so the
    /// many sync unit tests never panic); a push error is logged and dropped, so
    /// `process_step` is never failed by it. Uses `self.project_id` as the
    /// tenant — the same id the other families sync under.
    /// The project this engine belongs to — the reconcile filter's identity:
    /// only records stamped with this id merge in.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub(crate) fn cloud_push_last_step(&self) {
        let Some(client) = &self.cloud else {
            return;
        };
        let Some(step) = self.history.steps.last() else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let envelope = crate::cloud::build::from_step(&self.project_id, step);
        let client = client.clone();
        handle.spawn(async move {
            if let Err(e) = client.push(&envelope).await {
                tracing::warn!(target: "magistr_workflow::cloud", "think cloud push failed: {e}");
            }
        });
    }

    /// Attach a git-native trace sink so recorded steps are mirrored into the
    /// repo's `.think-and-ship/` as Agent Trace JSONL. `shared` selects the
    /// committed `sessions/` partition (`true`) vs the gitignored `local/`
    /// partition (`false`). Wired by the host's composition root when
    /// `THINK_AND_SHIP_SYNC_TARGET=repo-git` and the process is inside a repo.
    pub fn with_repo_sink(mut self, sink: crate::infra::RepoSink, shared: bool) -> Self {
        self.mirror = Some(crate::infra::MirrorWorker::spawn(sink));
        self.repo_shared = shared;
        self
    }

    /// Block until the git-native mirror has drained every step submitted so
    /// far. No-op without a mirror. For graceful shutdown and deterministic tests.
    pub fn flush_mirror(&self) {
        if let Some(m) = &self.mirror {
            m.flush();
        }
    }

    pub fn new(config: ThinkConfig) -> Self {
        let project_id = resolve_project_id();
        Self::new_for_project(config, project_id)
    }

    /// Construct the engine with an explicit project identity. The default
    /// history persists to `<project_id>.json` (think-trace-durability), so
    /// the caller should pass the same id it gives the ship/roadmap/signal
    /// engines — `new` resolves it from the environment for convenience.
    pub fn new_for_project(config: ThinkConfig, project_id: String) -> Self {
        let formatter = Formatter::new(config.display.color_output);
        let persistence = Persistence::for_project(&config.persistence, &project_id);

        let active_session = config.features.enable_sessions.then(|| {
            namespace_session_id(
                &project_id,
                config
                    .features
                    .default_session_id
                    .as_deref()
                    .unwrap_or(&project_id),
            )
        });
        let selected = match active_session.as_deref() {
            Some(session) => persistence.load_session(session),
            None => persistence.load_default(),
        };
        let selected_stored = selected.as_ref().is_ok_and(|read| read.value.is_some());
        let mut history = match selected {
            Ok(read) => read.value.unwrap_or_else(Self::new_history),
            Err(error) => {
                tracing::warn!(%error, "The reasoning archive could not be loaded");
                Self::new_history()
            }
        };
        let persisted_sessions = persistence.load_sessions().unwrap_or_else(|error| {
            tracing::warn!(%error, "Reasoning sessions could not be loaded");
            HashMap::new()
        });

        let mut sessions: HashMap<String, SessionEntry> = persisted_sessions
            .into_iter()
            .map(|(id, hist)| {
                (
                    id,
                    SessionEntry {
                        history: hist,
                        last_accessed: Self::now_ms(),
                    },
                )
            })
            .collect();

        if let Some(session) = &active_session {
            history.session_id = (session != &project_id).then(|| session.clone());
            if selected_stored {
                sessions.insert(
                    session.clone(),
                    SessionEntry {
                        history: history.clone(),
                        last_accessed: Self::now_ms(),
                    },
                );
            }
        }

        let cwd = std::env::current_dir()
            .ok()
            .map(|p| p.canonicalize().unwrap_or(p))
            .map(|p| p.display().to_string());

        let mut step_index: HashMap<u32, usize> = HashMap::new();
        let mut step_numbers: HashSet<u32> = HashSet::new();
        let mut step_to_branch: HashMap<u32, String> = HashMap::new();
        let mut tools_used_set: HashSet<String> = HashSet::new();
        let branches = Self::restored_branches(&history);

        for (idx, step) in history.steps.iter().enumerate() {
            step_index.insert(step.step_number, idx);
            step_numbers.insert(step.step_number);
            if let Some(bid) = &step.branch_id {
                step_to_branch.insert(step.step_number, bid.clone());
            }
        }
        if let Some(meta) = &history.metadata {
            if let Some(tools) = &meta.tools_used {
                for t in tools {
                    tools_used_set.insert(t.clone());
                }
            }
        }
        // Ensure metadata reflects loaded state.
        if let Some(meta) = history.metadata.as_mut() {
            meta.tools_used = Some({
                let mut v: Vec<String> = tools_used_set.iter().cloned().collect();
                v.sort();
                v
            });
        }

        let broadcaster = config.broadcast.path.clone().and_then(Broadcaster::spawn);

        Self {
            history,
            config,
            formatter,
            persistence,
            start_time: Instant::now(),
            branches,
            sessions,
            step_index,
            step_numbers,
            tools_used: tools_used_set,
            step_to_branch,
            branch_depth_cache: HashMap::new(),
            steps_since_cleanup: 0,
            branches_seq: 0,
            active_session,
            broadcaster,
            cwd,
            project_id,
            mirror: None,
            repo_shared: false,
            cloud: None,
        }
    }

    /// Build a read-only view of a history loaded from elsewhere — e.g.
    /// the GUI viewer parses a session file from disk and wants to call
    /// `impact_of` / `checkpoint_snapshot` / `recent_steps_rollup` on it.
    /// No persistence handle, no broadcaster, no disk I/O. The returned
    /// server's mutating methods still work but they only touch memory.
    pub fn for_analysis(history: ThinkHistory, branches: HashMap<String, Branch>) -> Self {
        let config = ThinkConfig::default();

        let mut step_index: HashMap<u32, usize> = HashMap::new();
        let mut step_numbers: HashSet<u32> = HashSet::new();
        let mut step_to_branch: HashMap<u32, String> = HashMap::new();
        let mut tools_used: HashSet<String> = HashSet::new();
        for (idx, step) in history.steps.iter().enumerate() {
            step_index.insert(step.step_number, idx);
            step_numbers.insert(step.step_number);
            if let Some(bid) = &step.branch_id {
                step_to_branch.insert(step.step_number, bid.clone());
            }
        }
        for branch in branches.values() {
            for step in &branch.steps {
                step_numbers.insert(step.step_number);
                step_to_branch.insert(step.step_number, branch.id.clone());
            }
        }
        if let Some(meta) = &history.metadata {
            if let Some(tools) = &meta.tools_used {
                for t in tools {
                    tools_used.insert(t.clone());
                }
            }
        }

        let formatter = Formatter::new(false);
        let persistence = Persistence::for_project(&config.persistence, "analysis");

        Self {
            history,
            config,
            formatter,
            persistence,
            start_time: Instant::now(),
            branches,
            sessions: HashMap::new(),
            step_index,
            step_numbers,
            tools_used,
            step_to_branch,
            branch_depth_cache: HashMap::new(),
            steps_since_cleanup: 0,
            branches_seq: 0,
            active_session: None,
            broadcaster: None,
            cwd: None,
            // for_analysis is read-only — no namespacing decisions get
            // made through this constructor, so an empty project id is
            // fine. Any path that would consult it (process_step) is
            // not reachable from a viewer-side `for_analysis` server.
            project_id: String::new(),
            // Read-only view: never mirrors to a repo.
            mirror: None,
            repo_shared: false,
            cloud: None,
        }
    }

    pub(crate) fn new_history() -> ThinkHistory {
        let now = Utc::now().to_rfc3339();
        ThinkHistory {
            steps: Vec::new(),
            branches: Some(Vec::new()),
            completed: false,
            session_id: None,
            created_at: Some(now.clone()),
            updated_at: Some(now),
            metadata: Some(HistoryMetadata {
                total_duration_ms: Some(0),
                revisions_count: Some(0),
                branches_created: Some(0),
                tools_used: Some(Vec::new()),
                project_id: None,
            }),
        }
    }

    /// Read the selected session's archive without changing this writer's memory.
    pub fn observe_archive(&self) -> Result<Observed<ThinkHistory>, ReadError> {
        if !self.persistence.enabled() {
            return Ok(Observed {
                value: self.history_with_branches(),
                source: ReadSource::Memory,
            });
        }
        let read = self.read_active_archive()?;
        let value = read.value.unwrap_or_else(|| ThinkHistory {
            steps: Vec::new(),
            branches: None,
            completed: false,
            session_id: self
                .active_session
                .clone()
                .filter(|id| id != &self.project_id),
            created_at: None,
            updated_at: None,
            metadata: None,
        });
        Ok(Observed {
            value,
            source: read.source,
        })
    }

    fn read_active_archive(&self) -> Result<Observed<Option<ThinkHistory>>, ReadError> {
        match self.active_session.as_deref() {
            Some(session) => self.persistence.load_session(session),
            None => self.persistence.load_default(),
        }
    }

    pub fn config(&self) -> &ThinkConfig {
        &self.config
    }

    pub fn history(&self) -> &ThinkHistory {
        &self.history
    }

    pub fn branches(&self) -> &HashMap<String, Branch> {
        &self.branches
    }

    pub fn sessions(&self) -> &HashMap<String, SessionEntry> {
        &self.sessions
    }

    // validate_thought_prefix, validate_rationale, validate_purpose
    // moved to `super::validation`.

    pub fn extract_tools_used(step: &ThinkStep) -> Vec<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<String> = Vec::new();
        if let Some(tools) = &step.tools_used {
            for t in tools {
                if seen.insert(t.clone()) {
                    out.push(t.clone());
                }
            }
        }
        if let NextAction::Structured(a) = &step.next_action {
            if let Some(t) = &a.tool {
                if seen.insert(t.clone()) {
                    out.push(t.clone());
                }
            }
        }
        out
    }

    // recover_xml_injection, validate_required_fields, validate_confidence
    // moved to `super::validation`.

    /// Merge steps pulled from the cloud into the loaded trace
    /// (sync-think-reconcile): insert-if-absent by `step_number` via the SAME
    /// `merge_histories` rule the disk merge uses — an existing local step is
    /// NEVER replaced, so a stale cloud copy cannot clobber local reasoning.
    /// A SILENT merge: no broadcast, no repo mirror, no cloud push-back (a
    /// reconcile that re-emitted would loop the pull straight into a push).
    /// Returns the number of steps adopted; persists only when > 0.
    ///
    /// The content guard holds against the FULL persisted archive, not just
    /// the trimmed in-memory window (adopt-archive-guard): the in-memory
    /// history is bounded by `max_history_size`, so a cloud clone of an older
    /// step would pass a memory-only check and re-pollute the disk on every
    /// reconcile. Incoming steps are pre-filtered against the archive's
    /// content keys and step numbers, and against each other (the cloud can
    /// hold several clones of the same step).
    pub fn adopt_steps(&mut self, steps: Vec<ThinkStep>) -> usize {
        if steps.is_empty() {
            return 0;
        }
        let mut seen: std::collections::HashSet<(String, String, String)> =
            std::collections::HashSet::new();
        let mut known_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut known_numbers: HashSet<u32> = HashSet::new();
        if self.persistence.enabled() {
            let archive = match self.read_active_archive() {
                Ok(read) => read.value,
                Err(error) => {
                    tracing::warn!(%error, "Reasoning adoption refused because the archive could not be read");
                    return 0;
                }
            };
            if let Some(archive) = archive {
                for step in &archive.steps {
                    if let Some(k) = step.content_key() {
                        seen.insert(k);
                    }
                    if let Some(id) = step
                        .record_id
                        .clone()
                        .or_else(|| step.backfilled_record_id())
                    {
                        known_ids.insert(id);
                    }
                    known_numbers.insert(step.step_number);
                }
            }
        }
        for step in self.history.steps.iter() {
            if let Some(id) = &step.record_id {
                known_ids.insert(id.clone());
            }
        }
        let mut steps = steps;
        for step in steps.iter_mut() {
            step.ensure_record_id();
        }
        let steps: Vec<ThinkStep> = steps
            .into_iter()
            .filter(|s| {
                if known_numbers.contains(&s.step_number) {
                    return false;
                }
                if let Some(id) = &s.record_id {
                    if !known_ids.insert(id.clone()) {
                        return false;
                    }
                }
                match s.content_key() {
                    Some(k) => seen.insert(k),
                    None => true,
                }
            })
            .collect();
        if steps.is_empty() {
            return 0;
        }
        let before = self.history.steps.len();
        let mut incoming = Self::new_history();
        incoming.steps = steps;
        incoming.branches = None;
        self.history = crate::think::persistence::merge_histories(&self.history, incoming);
        let adopted = self.history.steps.len() - before;
        if adopted > 0 {
            self.history
                .metadata
                .get_or_insert_with(Default::default)
                .project_id = Some(self.project_id.clone());
            self.persist_active();
        }
        adopted
    }

    /// Persist whatever history is currently loaded — the active session
    /// when one is set, otherwise the default history. Logs but doesn't
    /// propagate I/O errors so the in-memory state always stays consistent.
    pub(crate) fn persist_active(&self) {
        if !self.persistence.enabled() {
            return;
        }
        let snapshot = self.history_with_branches();
        match &self.active_session {
            Some(id) => self.persistence.save_session(id, &snapshot),
            None => self.persistence.save_default(&snapshot),
        }
    }
}

// Text helpers moved to `crate::think::util::text`.
// Recovery helpers moved to `crate::think::engine::recovery`.
