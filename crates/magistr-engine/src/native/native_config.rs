use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use magistr_workflow::infra::{PersistenceConfig, project_id_for_path};
use magistr_workflow::ship::application::EvidencePort;
use magistr_workflow::think::config::ThinkConfig;
use magistr_workflow::think::{ConfigSource, ThinkConfigResolution};

const PERSIST_VAR: &str = "MAGISTR_WORKFLOW_PERSIST";
const BROADCAST_VAR: &str = "MAGISTR_WORKFLOW_BROADCAST_PATH";
/// The socket `magistr mcp` binds under `.magistr/` when no
/// `MAGISTR_WORKFLOW_BROADCAST_PATH` is set, and in-process hosts (desktop, CLI)
/// subscribe to.
const PROJECT_SOCKET: &str = "workflow.sock";
/// The longest socket path the kernel accepts: `sun_path` minus its NUL —
/// 104 bytes on macOS (and the BSDs), 108 on Linux. A longer path fails
/// `bind(2)` with "path too long", and magistr's broadcaster then
/// runs unobserved.
const SUN_PATH_MAX: usize = if cfg!(target_os = "macos") { 103 } else { 107 };

/// Whether `bind(2)` accepts `path` on this platform.
pub(crate) fn fits_sun_path(path: &Path) -> bool {
    path.as_os_str().len() <= SUN_PATH_MAX
}

/// How the native magistr engines are opened.
///
/// Honours the same environment the external server reads —
/// `MAGISTR_WORKFLOW_DATA_DIR` (and the XDG fallback chain) for the store root,
/// `MAGISTR_WORKFLOW_BROADCAST_PATH` for the NDJSON socket — with one
/// deliberate difference: persistence is **on unless explicitly disabled**
/// (`MAGISTR_WORKFLOW_PERSIST=false|0`). The server defaults to off because a
/// stateless demo is a valid way to run it; a native subsystem that forgets
/// its roadmap at exit is not a valid way to run Magistr.
#[derive(Clone)]
pub struct NativeConfig {
    /// The project root the identity is resolved from.
    pub(super) project_root: PathBuf,
    /// Store root; each family persists under `<data_dir>/<family>/sessions`.
    pub(super) data_dir: PathBuf,
    /// Whether anything written survives the process.
    pub(super) persist: bool,
    /// Unix socket an external server broadcasts on, if configured.
    pub(super) broadcast_path: Option<PathBuf>,
    /// Short per-user directory for a socket that cannot live under the
    /// project root: `XDG_RUNTIME_DIR`, else `TMPDIR`, else `/tmp`.
    pub(super) runtime_dir: PathBuf,
    /// The host's verification evidence capability — source observation and
    /// the required gate contracts — composed into the ship family of every
    /// writer opened on this configuration. A writer opened without one
    /// records checks with no source and cannot complete a chunk; the host
    /// attaches it at every production admission.
    pub(super) evidence: Option<Arc<dyn EvidencePort>>,
    execution_id: String,
    pub(super) owner_generation: u64,
    think: ThinkConfig,
    project_name: Option<String>,
    sources: std::collections::BTreeMap<&'static str, ConfigSource>,
}

impl fmt::Debug for NativeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeConfig")
            .field("project_root", &self.project_root)
            .field("data_dir", &self.data_dir)
            .field("persist", &self.persist)
            .field("broadcast_path", &self.broadcast_path)
            .field("runtime_dir", &self.runtime_dir)
            .field("evidence", &self.evidence.is_some())
            .field("sources", &self.sources)
            .finish()
    }
}

impl PartialEq for NativeConfig {
    /// Two configurations are equal when they name the same store and
    /// project, captured behavior and provenance, and both carry (or both
    /// lack) an evidence capability; the capability itself has no identity to compare.
    fn eq(&self, other: &Self) -> bool {
        self.project_root == other.project_root
            && self.data_dir == other.data_dir
            && self.persist == other.persist
            && self.broadcast_path == other.broadcast_path
            && self.runtime_dir == other.runtime_dir
            && self.evidence.is_some() == other.evidence.is_some()
            && self.execution_id == other.execution_id
            && self.owner_generation == other.owner_generation
            && self.think == other.think
            && self.project_name == other.project_name
            && self.sources == other.sources
    }
}

impl Eq for NativeConfig {}

impl NativeConfig {
    /// Resolve from the process environment.
    #[must_use]
    pub fn from_env(project_root: impl Into<PathBuf>) -> Self {
        Self::resolve(
            project_root.into(),
            |key| std::env::var(key).ok(),
            Some(std::env::temp_dir()),
        )
    }

    /// Resolve through an injected lookup without touching process environment.
    /// Empty injected inputs use /tmp for the final store fallback; from_env
    /// captures the platform temporary directory instead. Supply an absolute
    /// project root for identity independent of later working-directory changes.
    #[must_use]
    pub fn from_lookup(
        project_root: impl Into<PathBuf>,
        get: impl Fn(&str) -> Option<String>,
    ) -> Self {
        Self::resolve(project_root.into(), get, None)
    }

    // Resolve one captured input set. Only from_env obtains the platform's
    // temporary directory; injected callers never fall through to host state.
    fn resolve(
        project_root: PathBuf,
        lookup: impl Fn(&str) -> Option<String>,
        platform_temp: Option<PathBuf>,
    ) -> Self {
        let captured = std::cell::RefCell::new(std::collections::BTreeMap::new());
        let get = |key: &str| {
            captured
                .borrow_mut()
                .entry(key.to_string())
                .or_insert_with(|| lookup(key))
                .clone()
        };
        let non_empty = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        let mut sources =
            std::collections::BTreeMap::from([("project_root", ConfigSource::Explicit)]);
        let persist = non_empty(PERSIST_VAR)
            .map(|v| {
                sources.insert(
                    "persistence.enabled",
                    ConfigSource::Environment(PERSIST_VAR),
                );
                !(v.trim().eq_ignore_ascii_case("false") || v.trim() == "0")
            })
            .unwrap_or(true);
        sources
            .entry("persistence.enabled")
            .or_insert(ConfigSource::Default);

        let data_candidates = [
            ("MAGISTR_WORKFLOW_DATA_DIR", ""),
            ("XDG_DATA_HOME", "magistr/workflow"),
            ("HOME", ".local/share/magistr/workflow"),
            ("APPDATA", "magistr/workflow"),
            ("USERPROFILE", "AppData/Roaming/magistr/workflow"),
        ];
        let data_dir = data_candidates
            .into_iter()
            .find_map(|(key, suffix)| {
                non_empty(key).map(|value| {
                    sources.insert("persistence.data_dir", ConfigSource::Environment(key));
                    if suffix.is_empty() {
                        PathBuf::from(value)
                    } else {
                        PathBuf::from(value).join(suffix)
                    }
                })
            })
            .unwrap_or_else(|| {
                // On Unix temp_dir uses TMPDIR; Windows also accepts TEMP/TMP.
                let keys: &[&'static str] = if cfg!(windows) {
                    &["TMPDIR", "TMP", "TEMP", "USERPROFILE"]
                } else {
                    &["TMPDIR"]
                };
                let temp = platform_temp.unwrap_or_else(|| {
                    keys.iter()
                        .find_map(|&key| {
                            non_empty(key).map(|value| {
                                sources
                                    .insert("persistence.data_dir", ConfigSource::Environment(key));
                                PathBuf::from(value)
                            })
                        })
                        .unwrap_or_else(|| PathBuf::from("/tmp"))
                });
                sources
                    .entry("persistence.data_dir")
                    .or_insert(ConfigSource::Default);
                temp.join("magistr/workflow")
            });
        let broadcast_path = non_empty(BROADCAST_VAR).map(|v| {
            sources.insert("broadcast.path", ConfigSource::Environment(BROADCAST_VAR));
            PathBuf::from(v.trim())
        });
        sources
            .entry("broadcast.path")
            .or_insert(ConfigSource::Default);
        let runtime_dir = ["XDG_RUNTIME_DIR", "TMPDIR"]
            .into_iter()
            .find_map(|key| {
                non_empty(key).map(|v| {
                    sources.insert("runtime_dir", ConfigSource::Environment(key));
                    PathBuf::from(v.trim())
                })
            })
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        sources
            .entry("runtime_dir")
            .or_insert(ConfigSource::Default);
        let project_name = non_empty("MAGISTR_PROJECT_ID").and_then(|raw| {
            let (name, source) =
                magistr_workflow::infra::resolve_project_id_from(Some(&raw), &project_root);
            (source == magistr_workflow::infra::IdSource::Environment).then_some(name)
        });
        sources.insert(
            "project_name",
            if project_name.is_some() {
                ConfigSource::Environment("MAGISTR_PROJECT_ID")
            } else {
                ConfigSource::Default
            },
        );
        let resolved = ThinkConfigResolution::from_lookup(
            get,
            PersistenceConfig {
                enabled: persist,
                data_dir: data_dir.clone(),
            },
        );
        // Native composition owns persistence and socket policy; think owns
        // only its parsed behavior settings.
        for (key, source) in resolved.sources {
            sources.entry(key).or_insert(source);
        }
        let execution_id = non_empty("MAGISTR_EXECUTION_ID");
        sources.insert(
            "execution_id",
            execution_id.as_ref().map_or(ConfigSource::Default, |_| {
                ConfigSource::Environment("MAGISTR_EXECUTION_ID")
            }),
        );
        Self {
            project_root,
            data_dir,
            persist,
            broadcast_path,
            runtime_dir,
            evidence: None,
            execution_id: execution_id.unwrap_or_else(|| "default".into()),
            owner_generation: 0,
            think: resolved.config,
            project_name,
            sources,
        }
    }

    /// Resolve the captured environment override and the project's pinned
    /// identity without consulting the process environment.
    #[must_use]
    pub fn resolve_project_id(&self) -> String {
        magistr_workflow::infra::resolve_project_id_from(
            self.project_name.as_deref(),
            &self.workflow_root(),
        )
        .0
    }

    /// Apply the host's canonical project root before admission.
    #[must_use]
    pub fn with_project_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.project_root = root.into();
        self.sources.insert("project_root", ConfigSource::Explicit);
        self
    }

    /// The shared project directory, distinct from this working copy's source root.
    #[must_use]
    pub fn workflow_root(&self) -> PathBuf {
        super::ProjectLocation::resolve(&self.project_root)
            .map(|location| location.authority_root)
            .unwrap_or_else(|_| self.project_root.clone())
    }

    #[must_use]
    pub fn execution_context(&self) -> super::ExecutionContext {
        super::ExecutionContext {
            working_copy: self
                .project_root
                .canonicalize()
                .unwrap_or_else(|_| self.project_root.clone()),
            execution_id: self.execution_id.clone(),
        }
    }

    #[must_use]
    pub fn with_execution_id(mut self, id: impl Into<String>) -> Self {
        self.execution_id = id.into();
        self.sources.insert("execution_id", ConfigSource::Explicit);
        self
    }

    pub(super) fn for_context(&self, context: &super::ExecutionContext) -> std::io::Result<Self> {
        let mut config = self
            .clone()
            .with_project_root(&context.working_copy)
            .with_execution_id(&context.execution_id);
        if context.working_copy != self.execution_context().working_copy {
            config.evidence = self
                .evidence
                .as_ref()
                .map(|port| port.for_working_copy(&context.working_copy))
                .transpose()
                .map_err(std::io::Error::other)?;
        }
        Ok(config)
    }

    /// The captured store root.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Whether native writes are persisted.
    #[must_use]
    pub fn persistence_enabled(&self) -> bool {
        self.persist
    }

    /// The configured publisher socket, before applying the project default.
    #[must_use]
    pub fn broadcast_path(&self) -> Option<&Path> {
        self.broadcast_path.as_deref()
    }

    /// The captured short-socket directory.
    #[must_use]
    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    /// Origins of captured settings and subsequent builder overrides.
    /// Contains source names, never environment values or credentials.
    #[must_use]
    pub fn configuration_sources(&self) -> &std::collections::BTreeMap<&'static str, ConfigSource> {
        &self.sources
    }

    /// Materialize captured behavior for the admitted project. The native
    /// publisher exclusively owns the socket; the think engine never binds it.
    #[must_use]
    pub fn think_config(&self, project_id: &str) -> ThinkConfig {
        let mut config = self.think.clone();
        config.persistence = self.persistence_config();
        config.broadcast.path = None;
        if config.features.enable_sessions && config.features.default_session_id.is_none() {
            config.features.default_session_id = Some(project_id.to_string());
        }
        config
    }

    /// Attach the host's verification evidence capability.
    #[must_use]
    pub fn with_evidence(mut self, evidence: Arc<dyn EvidencePort>) -> Self {
        self.evidence = Some(evidence);
        self
    }

    /// The evidence capability writers opened on this configuration carry.
    #[must_use]
    pub fn evidence(&self) -> Option<&Arc<dyn EvidencePort>> {
        self.evidence.as_ref()
    }

    /// Point the store at `dir` — what every test does.
    #[must_use]
    pub fn with_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.data_dir = dir.into();
        self.sources
            .insert("persistence.data_dir", ConfigSource::Explicit);
        self
    }

    /// Toggle persistence.
    #[must_use]
    pub fn persist(mut self, on: bool) -> Self {
        self.persist = on;
        self.sources
            .insert("persistence.enabled", ConfigSource::Explicit);
        self
    }

    /// Set the broadcast socket path.
    #[must_use]
    pub fn with_broadcast_path(mut self, path: Option<PathBuf>) -> Self {
        self.broadcast_path = path;
        self.sources
            .insert("broadcast.path", ConfigSource::Explicit);
        self
    }

    /// Point the short socket directory at `dir` — where an owner endpoint
    /// or feed socket lands when its store is rooted too deep for
    /// `sun_path`.
    #[must_use]
    pub fn with_runtime_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.runtime_dir = dir.into();
        self.sources.insert("runtime_dir", ConfigSource::Explicit);
        self
    }

    /// The project's default broadcast socket — the well-known rendezvous
    /// between `magistr mcp` (which binds it when the environment names
    /// no other socket) and the in-process hosts that subscribe to it.
    ///
    /// `<project_root>/.magistr/workflow.sock` whenever that fits the
    /// kernel's `sun_path`; a project rooted deep enough to overflow it gets
    /// `<runtime_dir>/magistr-<project id>.sock` instead, where the id is
    /// magistr's stable path-derived identity, so every process
    /// resolves the same file from the same root without a rendezvous file.
    #[must_use]
    pub fn project_socket_path(&self) -> PathBuf {
        let local = self.workflow_root().join(".magistr").join(PROJECT_SOCKET);
        if fits_sun_path(&local) {
            return local;
        }
        let root = self.workflow_root();
        let id = project_id_for_path(&root);
        let short = self.runtime_dir.join(format!("magistr-{id}.sock"));
        if fits_sun_path(&short) {
            return short;
        }
        // Even the runtime dir is too deep: keep only the hash.
        let hash = id.rsplit('-').next().unwrap_or(&id);
        PathBuf::from("/tmp").join(format!("magistr-{hash}.sock"))
    }

    /// The socket a subscriber (TUI native feed, GUI) should connect to: the
    /// environment's `MAGISTR_WORKFLOW_BROADCAST_PATH` when set, else
    /// [`Self::project_socket_path`] — exactly what `magistr mcp` binds.
    #[must_use]
    pub fn feed_socket_path(&self) -> PathBuf {
        self.broadcast_path
            .clone()
            .unwrap_or_else(|| self.project_socket_path())
    }

    /// Fall back to [`Self::project_socket_path`] when the environment named
    /// no broadcast socket. An explicit `MAGISTR_WORKFLOW_BROADCAST_PATH` wins.
    #[must_use]
    pub fn with_project_socket_default(mut self) -> Self {
        if self.broadcast_path.is_none() {
            self.broadcast_path = Some(self.project_socket_path());
            self.sources
                .insert("broadcast.path", ConfigSource::ProjectIdentity);
        }
        self
    }

    /// The library's persistence config for this store.
    #[must_use]
    pub fn persistence_config(&self) -> PersistenceConfig {
        PersistenceConfig {
            enabled: self.persist,
            data_dir: self.data_dir.clone(),
        }
    }

    /// The project root as a path.
    #[must_use]
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}

#[cfg(test)]
#[path = "native_config.tests.rs"]
mod tests;
