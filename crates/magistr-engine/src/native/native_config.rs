use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use magistr_workflow::infra::{PersistenceConfig, project_id_for_path};
use magistr_workflow::ship::application::EvidencePort;

const PERSIST_VAR: &str = "THINK_AND_SHIP_PERSIST";
const BROADCAST_VAR: &str = "THINK_AND_SHIP_BROADCAST_PATH";
/// The socket `magistr mcp-serve` binds under `.magistr/` when no
/// `THINK_AND_SHIP_BROADCAST_PATH` is set, and in-process hosts (TUI, GUI)
/// subscribe to.
const PROJECT_SOCKET: &str = "think-and-ship.sock";
/// The longest socket path the kernel accepts: `sun_path` minus its NUL —
/// 104 bytes on macOS (and the BSDs), 108 on Linux. A longer path fails
/// `bind(2)` with "path too long", and think-and-ship's broadcaster then
/// runs unobserved.
const SUN_PATH_MAX: usize = if cfg!(target_os = "macos") { 103 } else { 107 };

/// Whether `bind(2)` accepts `path` on this platform.
pub(crate) fn fits_sun_path(path: &Path) -> bool {
    path.as_os_str().len() <= SUN_PATH_MAX
}

/// How the native think-and-ship engines are opened.
///
/// Honours the same environment the external server reads —
/// `THINK_AND_SHIP_DATA_DIR` (and the XDG fallback chain) for the store root,
/// `THINK_AND_SHIP_BROADCAST_PATH` for the NDJSON socket — with one
/// deliberate difference: persistence is **on unless explicitly disabled**
/// (`THINK_AND_SHIP_PERSIST=false|0`). The server defaults to off because a
/// stateless demo is a valid way to run it; a native subsystem that forgets
/// its roadmap at exit is not a valid way to run Magistr.
#[derive(Clone)]
pub struct NativeConfig {
    /// The project root the identity is resolved from.
    pub project_root: PathBuf,
    /// Store root; each family persists under `<data_dir>/<family>/sessions`.
    pub data_dir: PathBuf,
    /// Whether anything written survives the process.
    pub persist: bool,
    /// Unix socket an external server broadcasts on, if configured.
    pub broadcast_path: Option<PathBuf>,
    /// Short per-user directory for a socket that cannot live under the
    /// project root: `XDG_RUNTIME_DIR`, else `TMPDIR`, else `/tmp`.
    pub runtime_dir: PathBuf,
    /// The host's verification evidence capability — source observation and
    /// the required gate contracts — composed into the ship family of every
    /// writer opened on this configuration. A writer opened without one
    /// records checks with no source and cannot complete a chunk; the host
    /// attaches it at every production admission.
    pub evidence: Option<Arc<dyn EvidencePort>>,
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
            .finish()
    }
}

impl PartialEq for NativeConfig {
    /// Two configurations are equal when they name the same store and
    /// project and both carry (or both lack) an evidence capability; the
    /// capability itself has no identity to compare.
    fn eq(&self, other: &Self) -> bool {
        self.project_root == other.project_root
            && self.data_dir == other.data_dir
            && self.persist == other.persist
            && self.broadcast_path == other.broadcast_path
            && self.runtime_dir == other.runtime_dir
            && self.evidence.is_some() == other.evidence.is_some()
    }
}

impl Eq for NativeConfig {}

impl NativeConfig {
    /// Resolve from the process environment.
    #[must_use]
    pub fn from_env(project_root: impl Into<PathBuf>) -> Self {
        Self::from_lookup(project_root, |key| std::env::var(key).ok())
    }

    /// Resolve through an injected lookup so tests can drive every arm
    /// without touching process environment.
    #[must_use]
    pub fn from_lookup(
        project_root: impl Into<PathBuf>,
        get: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let non_empty = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        let persist = non_empty(PERSIST_VAR)
            .map(|v| {
                let v = v.trim();
                !(v.eq_ignore_ascii_case("false") || v == "0")
            })
            .unwrap_or(true);
        let data_dir = non_empty("THINK_AND_SHIP_DATA_DIR")
            .map(PathBuf::from)
            // Match think-and-ship 0.6's fallback chain through the same
            // lookup. Calling PersistenceConfig::from_env here would leak
            // the host's store into otherwise injected configurations.
            .or_else(|| non_empty("XDG_DATA_HOME").map(|v| PathBuf::from(v).join("think-and-ship")))
            .or_else(|| {
                non_empty("HOME").map(|v| PathBuf::from(v).join(".local/share/think-and-ship"))
            })
            .or_else(|| non_empty("APPDATA").map(|v| PathBuf::from(v).join("think-and-ship")))
            .or_else(|| {
                non_empty("USERPROFILE")
                    .map(|v| PathBuf::from(v).join("AppData/Roaming/think-and-ship"))
            })
            .unwrap_or_else(|| std::env::temp_dir().join("think-and-ship"));
        let broadcast_path = non_empty(BROADCAST_VAR).map(|v| PathBuf::from(v.trim()));
        let runtime_dir = non_empty("XDG_RUNTIME_DIR")
            .or_else(|| non_empty("TMPDIR"))
            .map(|v| PathBuf::from(v.trim()))
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        Self {
            project_root: project_root.into(),
            data_dir,
            persist,
            broadcast_path,
            runtime_dir,
            evidence: None,
        }
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
        self
    }

    /// Toggle persistence.
    #[must_use]
    pub fn persist(mut self, on: bool) -> Self {
        self.persist = on;
        self
    }

    /// Set the broadcast socket path.
    #[must_use]
    pub fn with_broadcast_path(mut self, path: Option<PathBuf>) -> Self {
        self.broadcast_path = path;
        self
    }

    /// Point the short socket directory at `dir` — where an owner endpoint
    /// or feed socket lands when its store is rooted too deep for
    /// `sun_path`.
    #[must_use]
    pub fn with_runtime_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.runtime_dir = dir.into();
        self
    }

    /// The project's default broadcast socket — the well-known rendezvous
    /// between `magistr mcp-serve` (which binds it when the environment names
    /// no other socket) and the in-process hosts that subscribe to it.
    ///
    /// `<project_root>/.magistr/think-and-ship.sock` whenever that fits the
    /// kernel's `sun_path`; a project rooted deep enough to overflow it gets
    /// `<runtime_dir>/magistr-<project id>.sock` instead, where the id is
    /// think-and-ship's stable path-derived identity, so every process
    /// resolves the same file from the same root without a rendezvous file.
    #[must_use]
    pub fn project_socket_path(&self) -> PathBuf {
        let local = self.project_root.join(".magistr").join(PROJECT_SOCKET);
        if fits_sun_path(&local) {
            return local;
        }
        let root = self
            .project_root
            .canonicalize()
            .unwrap_or_else(|_| self.project_root.clone());
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
    /// environment's `THINK_AND_SHIP_BROADCAST_PATH` when set, else
    /// [`Self::project_socket_path`] — exactly what `magistr mcp-serve` binds.
    #[must_use]
    pub fn feed_socket_path(&self) -> PathBuf {
        self.broadcast_path
            .clone()
            .unwrap_or_else(|| self.project_socket_path())
    }

    /// Fall back to [`Self::project_socket_path`] when the environment named
    /// no broadcast socket. An explicit `THINK_AND_SHIP_BROADCAST_PATH` wins.
    #[must_use]
    pub fn with_project_socket_default(mut self) -> Self {
        if self.broadcast_path.is_none() {
            self.broadcast_path = Some(self.project_socket_path());
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
