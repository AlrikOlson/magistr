use std::{collections::BTreeMap, path::PathBuf};

use crate::{config::ResolveOptions, transaction::FormatterOptions, worker::WorkerOptions};

/// What the composing host decides before a workspace opens. Environment
/// lookups are explicit: `from_environment` reads the retained variables,
/// `new` inherits nothing.
#[derive(Debug, Clone)]
pub struct WorkspaceOpenOptions {
    pub root: PathBuf,
    /// The executable serving the private worker role (production: this binary).
    pub worker_executable: PathBuf,
    pub worker_options: WorkerOptions,
    /// Configuration environment: `WORKBENCH_ORG_POLICY`, `WORKBENCH_USER_CONFIG`,
    /// `XDG_CONFIG_HOME`, `HOME`, `USERPROFILE`, `ProgramData`.
    pub env: BTreeMap<String, String>,
    /// Ephemeral highest-precedence `--config key=value` flags.
    pub session_flags: serde_json::Map<String, serde_json::Value>,
    /// `WORKBENCH_NETWORK_FS`: "1" forces network, "0" forces local.
    pub network_override: Option<String>,
    pub formatter: FormatterOptions,
    /// Stable shared lock retained while this source owner keeps its containing working copy open.
    pub lifecycle_lock: Option<PathBuf>,
    /// The retained QuickJS-ng interpreter; absent, the script verb is not offered.
    #[cfg(feature = "sandbox")]
    pub interpreter: Option<crate::commands::ScriptInterpreter>,
}

impl WorkspaceOpenOptions {
    /// Explicit inputs only: no environment is consulted.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, worker_executable: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            worker_executable: worker_executable.into(),
            worker_options: WorkerOptions::default(),
            env: BTreeMap::new(),
            session_flags: serde_json::Map::new(),
            network_override: None,
            formatter: FormatterOptions::default(),
            lifecycle_lock: None,
            #[cfg(feature = "sandbox")]
            interpreter: None,
        }
    }

    /// The host's environment, read once: configuration layers and the
    /// network-filesystem override.
    #[must_use]
    pub fn from_environment(
        root: impl Into<PathBuf>,
        worker_executable: impl Into<PathBuf>,
    ) -> Self {
        let root = root.into();
        let env = ResolveOptions::from_environment(&root).env;
        Self {
            env,
            network_override: std::env::var("WORKBENCH_NETWORK_FS").ok(),
            ..Self::new(root, worker_executable)
        }
    }

    #[must_use]
    pub fn with_session_flags(mut self, flags: serde_json::Map<String, serde_json::Value>) -> Self {
        self.session_flags = flags;
        self
    }

    #[must_use]
    pub fn with_network_override(mut self, value: impl Into<String>) -> Self {
        self.network_override = Some(value.into());
        self
    }

    #[must_use]
    pub fn with_worker_options(mut self, options: WorkerOptions) -> Self {
        self.worker_options = options;
        self
    }

    #[must_use]
    pub fn with_formatter(mut self, formatter: FormatterOptions) -> Self {
        self.formatter = formatter;
        self
    }

    #[cfg(feature = "sandbox")]
    #[must_use]
    pub fn with_interpreter(mut self, interpreter: crate::commands::ScriptInterpreter) -> Self {
        self.interpreter = Some(interpreter);
        self
    }

    /// Whether an owner opened from these options offers the script verb:
    /// an interpreter is composed and the sandbox host is built in. The
    /// opened service answers the same from the interpreter it kept.
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

    pub(super) fn resolve_options(&self, root: PathBuf) -> ResolveOptions {
        ResolveOptions {
            root,
            env: self.env.clone(),
            session_flags: self.session_flags.clone(),
        }
    }
}
