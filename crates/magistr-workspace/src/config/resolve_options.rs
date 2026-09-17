use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Explicit inputs to read-only config resolution. An empty environment never
/// inherits ambient overrides; `from_environment` opts into host lookup.
#[derive(Debug, Clone)]
pub struct ResolveOptions {
    pub root: PathBuf,
    pub env: BTreeMap<String, String>,
    pub session_flags: serde_json::Map<String, serde_json::Value>,
}

impl ResolveOptions {
    /// Explicit-input resolution; absent HOME/USERPROFILE uses the workspace
    /// root for the fallback user-config base. Hosts use `from_environment`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            env: BTreeMap::new(),
            session_flags: Default::default(),
        }
    }

    #[must_use]
    pub fn from_environment(root: impl Into<PathBuf>) -> Self {
        let env = [
            "WORKBENCH_ORG_POLICY",
            "WORKBENCH_USER_CONFIG",
            "ProgramData",
            "XDG_CONFIG_HOME",
            "HOME",
            "USERPROFILE",
        ]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| (key.into(), value)))
        .collect();
        Self {
            root: root.into(),
            env,
            session_flags: Default::default(),
        }
    }

    /// Paths are exposed so the write-admission owner can protect every layer.
    #[must_use]
    pub fn layer_paths(&self) -> [(super::ConfigLayer, PathBuf); 3] {
        use super::ConfigLayer;
        [
            (ConfigLayer::OrgPolicy, self.org_policy_path()),
            (ConfigLayer::User, self.user_config_path()),
            (ConfigLayer::Workspace, self.root.join("workbench.toml")),
        ]
    }

    /// The layers that resolve outside the workspace tree, in precedence
    /// order: the organization policy and the user layer. The workspace's
    /// own `workbench.toml` is a source input of a snapshot, never an
    /// external layer.
    #[must_use]
    pub fn external_layer_paths(&self) -> [(super::ConfigLayer, PathBuf); 2] {
        use super::ConfigLayer;
        [
            (ConfigLayer::OrgPolicy, self.org_policy_path()),
            (ConfigLayer::User, self.user_config_path()),
        ]
    }

    fn value(&self, key: &str) -> Option<&str> {
        self.env
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    fn org_policy_path(&self) -> PathBuf {
        if let Some(path) = self.value("WORKBENCH_ORG_POLICY") {
            return path.into();
        }
        if cfg!(windows) {
            Path::new(self.value("ProgramData").unwrap_or("C:\\ProgramData"))
                .join("workbench")
                .join("policy.toml")
        } else {
            PathBuf::from("/etc/workbench/policy.toml")
        }
    }

    fn user_config_path(&self) -> PathBuf {
        if let Some(path) = self.value("WORKBENCH_USER_CONFIG") {
            return path.into();
        }
        let base = self
            .value("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                self.value("HOME")
                    .or_else(|| self.value("USERPROFILE"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.root.clone())
                    .join(".config")
            });
        base.join("workbench").join("config.toml")
    }
}
