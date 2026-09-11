use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// What a doctor run reads: the workspace, its state directory, the
/// explicit environment and session flags for config resolution, the tool
/// version to stamp and the clock. Nothing here is written back.
#[derive(Debug, Clone)]
pub struct DoctorOptions {
    pub root: PathBuf,
    pub state_dir: PathBuf,
    pub env: BTreeMap<String, String>,
    pub session_flags: serde_json::Map<String, serde_json::Value>,
    pub tool_version: String,
    pub now_unix_ms: u64,
}

impl DoctorOptions {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = root.into();
        Self {
            state_dir: root.join(".workbench"),
            root,
            env: BTreeMap::new(),
            session_flags: serde_json::Map::new(),
            tool_version: crate::VERSION.to_owned(),
            now_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis() as u64)
                .unwrap_or(0),
        }
    }

    #[must_use]
    pub fn with_state_dir(mut self, state_dir: impl Into<PathBuf>) -> Self {
        self.state_dir = state_dir.into();
        self
    }

    #[must_use]
    pub fn with_env(mut self, env: BTreeMap<String, String>) -> Self {
        self.env = env;
        self
    }

    #[must_use]
    pub fn with_session_flags(mut self, flags: serde_json::Map<String, serde_json::Value>) -> Self {
        self.session_flags = flags;
        self
    }

    #[must_use]
    pub fn with_tool_version(mut self, version: impl Into<String>) -> Self {
        self.tool_version = version.into();
        self
    }

    #[must_use]
    pub fn with_now_unix_ms(mut self, now_unix_ms: u64) -> Self {
        self.now_unix_ms = now_unix_ms;
        self
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}
