//! Core reasoning-step engine. Owns the `ReasoningServer` struct and its sibling impl blocks.

use magistr_workflow_contracts::{Observed, ReadSource, ReadState};

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
use crate::think::persistence::{PersistedProject, Persistence};

pub struct ReasoningServer {
    // Fields are `pub(crate)` so sibling modules under `engine::*` can
    // share access without bouncing through accessors. Anything outside
    // this crate still goes through the `config()` / `history()` /
    // `branches()` / `sessions()` accessors.
    pub(crate) history: ThinkHistory,
    /// Complete owner state; history and session indexes are derived views.
    pub(crate) project: PersistedProject,
    pub(crate) observation: ReadState<()>,
    pub(crate) config: ThinkConfig,
    pub(crate) formatter: Formatter,
    pub(crate) persistence: Persistence,
    pub(crate) start_time: Instant,
    pub(crate) branches: HashMap<String, Branch>,
    pub(crate) sessions: HashMap<String, SessionEntry>,
    pub(crate) step_index: HashMap<u32, usize>,
    pub(crate) step_numbers: HashSet<u32>,
    /// Index into the complete canonical archive, independent of display retention.
    pub(crate) project_steps: HashMap<u32, (String, usize)>,
    pub(crate) project_max_step: u32,
    pub(crate) append_state_validated: bool,
    pub(crate) pinned_order: Vec<u32>,
    pub(crate) tools_used: HashSet<String>,
    pub(crate) step_to_branch: HashMap<u32, String>,
    pub(crate) branch_depth_cache: HashMap<u32, u32>,
    pub(crate) steps_since_cleanup: u32,
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
    /// `MAGISTR_PROJECT_ID` override). Used to namespace every
    /// caller-supplied session id and stamped into each session's
    /// metadata so the viewer can group without parsing.
    pub(crate) project_id: String,
    /// Optional git-native trace mirror. When set (SyncTarget::RepoGit + the
    /// process runs inside a repo), every recorded step is mirrored as an
    /// Agent Trace JSONL record into `.magistr/traces/` and the session is
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

    pub(crate) fn cloud_push_step(&self, step: &ThinkStep) {
        let Some(client) = &self.cloud else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Ok(envelope) = crate::cloud::build::from_step(&self.project_id, step) else {
            return;
        };
        let client = client.clone();
        handle.spawn(async move {
            if let Err(e) = client.push(&envelope).await {
                tracing::warn!(target: "magistr_workflow::cloud", "think cloud push failed: {e}");
            }
        });
    }

    /// Attach a git-native trace sink so recorded steps are mirrored into the
    /// repo's `.magistr/traces/` as Agent Trace JSONL. `shared` selects the
    /// committed `sessions/` partition (`true`) vs the gitignored `local/`
    /// partition (`false`). Wired by the host's composition root when
    /// `MAGISTR_WORKFLOW_SYNC_TARGET=repo-git` and the process is inside a repo.
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
        let observed = persistence.load_project();
        let mut observation = ReadState::default();
        observation.record(
            observed
                .as_ref()
                .map(|read| Observed {
                    value: (),
                    source: read.source,
                })
                .map_err(Clone::clone),
        );
        let project = observed
            .ok()
            .and_then(|read| read.value)
            .unwrap_or_else(|| PersistedProject::empty(&project_id));
        let broadcaster = config.broadcast.path.clone().and_then(Broadcaster::spawn);
        let cwd = std::env::current_dir()
            .ok()
            .map(|p| p.canonicalize().unwrap_or(p))
            .map(|p| p.display().to_string());
        let mut server = Self {
            history: Self::new_history(),
            project: project.clone(),
            observation,
            formatter: Formatter::new(config.display.color_output),
            config,
            persistence,
            start_time: Instant::now(),
            branches: HashMap::new(),
            sessions: HashMap::new(),
            step_index: HashMap::new(),
            step_numbers: HashSet::new(),
            project_steps: HashMap::new(),
            project_max_step: 0,
            append_state_validated: true,
            pinned_order: Vec::new(),
            tools_used: HashSet::new(),
            step_to_branch: HashMap::new(),
            branch_depth_cache: HashMap::new(),
            steps_since_cleanup: 0,
            active_session: active_session.clone(),
            broadcaster,
            cwd,
            project_id,
            mirror: None,
            repo_shared: false,
            cloud: None,
        };
        server.install_project(project, active_session, false);
        server
    }

    /// Build a read-only view of a history loaded from elsewhere — e.g.
    /// the GUI viewer parses a session file from disk and wants to call
    /// `impact_of` / `checkpoint_snapshot` / `recent_steps_rollup` on it.
    /// No persistence handle, no broadcaster, no disk I/O. The returned
    /// server's mutating methods still work but they only touch memory.
    pub fn for_analysis(mut history: ThinkHistory, branches: HashMap<String, Branch>) -> Self {
        let config = ThinkConfig::default();
        let project_id = history
            .metadata
            .as_ref()
            .and_then(|meta| meta.project_id.clone())
            .unwrap_or_else(|| "analysis".into());
        history
            .metadata
            .get_or_insert_with(Default::default)
            .project_id = Some(project_id.clone());
        let mut project = PersistedProject::empty(&project_id);
        project.histories.insert(
            history
                .session_id
                .clone()
                .unwrap_or_else(|| project_id.clone()),
            history.clone(),
        );

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

        let active_session = history.session_id.clone();
        let formatter = Formatter::new(false);
        let persistence = Persistence::for_project(&config.persistence, &project_id);

        let project_steps = project
            .histories
            .iter()
            .flat_map(|(key, history)| {
                history
                    .steps
                    .iter()
                    .enumerate()
                    .map(move |(index, step)| (step.step_number, (key.clone(), index)))
            })
            .collect();
        let project_max_step = history
            .steps
            .iter()
            .map(|step| step.step_number)
            .max()
            .unwrap_or(0);
        let pinned_order = history
            .steps
            .iter()
            .filter(|step| step.pinned.unwrap_or(false))
            .map(|step| step.step_number)
            .collect();

        Self {
            history,
            project,
            project_steps,
            project_max_step,
            append_state_validated: false,
            pinned_order,
            observation: ReadState::Current(Observed {
                value: (),
                source: ReadSource::Memory,
            }),
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
            active_session,
            broadcaster: None,
            cwd: None,
            project_id,
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
}
