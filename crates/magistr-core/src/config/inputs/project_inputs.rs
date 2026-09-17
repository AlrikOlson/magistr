use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use magistr_workspace::config::ResolveOptions;

use super::{CapturedInput, InputSource};
use crate::config::preset_full::LayerOptions;
use crate::config::project_preset::{PROJECT_PRESET_DIR, PresetInfo, list_presets_in_dir};
use crate::config::user_config::{self, UserConfig, UserConfigError};
use crate::config::user_preset::load_user_override_from_dir;
use crate::error::{MagistrError, Result};
use crate::types::{Language, PresetOverride, ProjectConfig};

const XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
const HOME: &str = "HOME";
const AGENT_BACKEND: &str = "MAGISTR_AGENT_BACKEND";
const CLAUDE_API_KEY: &str = "ANTHROPIC_API_KEY";
const PLUGIN_INDEX_URL: &str = "MAGISTR_PLUGIN_INDEX_URL";
const WORKSPACE_NETWORK_FS: &str = "WORKBENCH_NETWORK_FS";
const WORKSPACE_SESSION_ID: &str = "WORKBENCH_SESSION_ID";
/// The environment the workspace's configuration layers resolve from —
/// exactly the keys `ResolveOptions::from_environment` reads.
const WORKSPACE_ENV: [(&str, &str); 6] = [
    ("WORKBENCH_ORG_POLICY", "workspace.org_policy"),
    ("WORKBENCH_USER_CONFIG", "workspace.user_config"),
    ("ProgramData", "workspace.program_data"),
    ("XDG_CONFIG_HOME", "workspace.xdg_config_home"),
    ("HOME", "workspace.home"),
    ("USERPROFILE", "workspace.user_profile"),
];

/// Magistr's per-user directory under the XDG config base.
const USER_DIR: &str = "magistr";
const USER_CONFIG_FILE: &str = "config.toml";
const USER_PRESET_DIR: &str = "presets";
const MODELS_FILE: &str = "models.toml";
const PLUGINS_DIR: &str = "plugins";
const ORG_PLUGIN_CONFIG_FILE: &str = "org-plugins.json";

/// Project, preset, provider, plugin and workspace inputs captured once at
/// composition. Use cases resolve paths and load files from this set; they
/// never consult the process environment again.
///
/// Secrets are never captured: the Claude API key is recorded only as
/// present or absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInputs {
    project_root: PathBuf,
    /// `$XDG_CONFIG_HOME/magistr`, else `$HOME/.config/magistr`, else none.
    config_dir: Option<PathBuf>,
    agent_backend: Option<String>,
    claude_api_key: bool,
    plugin_index_url: Option<String>,
    workspace_env: BTreeMap<String, String>,
    workspace_network_override: Option<String>,
    workspace_session_id: Option<String>,
    sources: BTreeMap<&'static str, InputSource>,
}

impl ProjectInputs {
    /// Capture from the process environment — the production boundary.
    #[must_use]
    pub fn from_env(project_root: impl Into<PathBuf>) -> Self {
        Self::from_lookup(project_root, |key| std::env::var(key).ok())
    }

    /// Capture through an injected lookup without touching the process
    /// environment. Each key is asked for at most once. Blank values count
    /// as unset, as the XDG specification prescribes.
    #[must_use]
    pub fn from_lookup(
        project_root: impl Into<PathBuf>,
        get: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let captured = std::cell::RefCell::new(BTreeMap::<String, Option<String>>::new());
        let value = |key: &str| -> Option<String> {
            captured
                .borrow_mut()
                .entry(key.to_string())
                .or_insert_with(|| get(key))
                .clone()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let mut sources = BTreeMap::from([("project_root", InputSource::Explicit)]);

        let config_dir = value(XDG_CONFIG_HOME)
            .map(|xdg| {
                sources.insert("user.config_dir", InputSource::Environment(XDG_CONFIG_HOME));
                PathBuf::from(xdg).join(USER_DIR)
            })
            .or_else(|| {
                value(HOME).map(|home| {
                    sources.insert("user.config_dir", InputSource::Environment(HOME));
                    PathBuf::from(home).join(".config").join(USER_DIR)
                })
            });
        sources
            .entry("user.config_dir")
            .or_insert(InputSource::Default);

        let agent_backend = value(AGENT_BACKEND);
        sources.insert(
            "provider.backend",
            Self::source_for(agent_backend.is_some(), AGENT_BACKEND),
        );
        let claude_api_key = value(CLAUDE_API_KEY).is_some();
        sources.insert(
            "provider.claude_api_key",
            Self::source_for(claude_api_key, CLAUDE_API_KEY),
        );
        let plugin_index_url = value(PLUGIN_INDEX_URL);
        sources.insert(
            "plugin.index_url",
            Self::source_for(plugin_index_url.is_some(), PLUGIN_INDEX_URL),
        );

        let mut workspace_env = BTreeMap::new();
        for (key, setting) in WORKSPACE_ENV {
            if let Some(v) = value(key) {
                workspace_env.insert(key.to_string(), v);
                sources.insert(setting, InputSource::Environment(key));
            } else {
                sources.insert(setting, InputSource::Default);
            }
        }
        let workspace_network_override = value(WORKSPACE_NETWORK_FS);
        sources.insert(
            "workspace.network_fs",
            Self::source_for(workspace_network_override.is_some(), WORKSPACE_NETWORK_FS),
        );
        let workspace_session_id = value(WORKSPACE_SESSION_ID);
        sources.insert(
            "workspace.session_id",
            Self::source_for(workspace_session_id.is_some(), WORKSPACE_SESSION_ID),
        );

        Self {
            project_root: project_root.into(),
            config_dir,
            agent_backend,
            claude_api_key,
            plugin_index_url,
            workspace_env,
            workspace_network_override,
            workspace_session_id,
            sources,
        }
    }

    fn source_for(present: bool, key: &'static str) -> InputSource {
        if present {
            InputSource::Environment(key)
        } else {
            InputSource::Default
        }
    }

    // ── Overrides ────────────────────────────────────────────────────

    /// The same captured user/provider/plugin/workspace inputs re-rooted at
    /// another project — what a `--dir` command uses.
    #[must_use]
    pub fn for_project(&self, root: impl Into<PathBuf>) -> Self {
        let mut inputs = self.clone();
        inputs.project_root = root.into();
        inputs.sources.insert("project_root", InputSource::Explicit);
        inputs
    }

    /// Point the per-user configuration directory somewhere explicit — what
    /// a test does instead of setting `XDG_CONFIG_HOME`.
    #[must_use]
    pub fn with_config_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config_dir = Some(dir.into());
        self.sources
            .insert("user.config_dir", InputSource::Explicit);
        self
    }

    /// Override the captured provider backend request.
    #[must_use]
    pub fn with_agent_backend(mut self, backend: Option<String>) -> Self {
        self.agent_backend = backend
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        self.sources
            .insert("provider.backend", InputSource::Explicit);
        self
    }

    /// Override whether a Claude API key is available.
    #[must_use]
    pub fn with_claude_api_key(mut self, present: bool) -> Self {
        self.claude_api_key = present;
        self.sources
            .insert("provider.claude_api_key", InputSource::Explicit);
        self
    }

    // ── Captured values ──────────────────────────────────────────────

    /// The project root the inputs are bound to.
    #[must_use]
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Magistr's per-user configuration directory, when one could be resolved.
    #[must_use]
    pub fn config_dir(&self) -> Option<&Path> {
        self.config_dir.as_deref()
    }

    /// `MAGISTR_AGENT_BACKEND`, trimmed and nonblank.
    #[must_use]
    pub fn agent_backend(&self) -> Option<&str> {
        self.agent_backend.as_deref()
    }

    /// Whether `ANTHROPIC_API_KEY` was set and nonblank at capture.
    #[must_use]
    pub fn has_claude_api_key(&self) -> bool {
        self.claude_api_key
    }

    /// `MAGISTR_PLUGIN_INDEX_URL`, trimmed and nonblank.
    #[must_use]
    pub fn plugin_index_url(&self) -> Option<&str> {
        self.plugin_index_url.as_deref()
    }

    /// The environment the workspace's configuration layers resolve from.
    #[must_use]
    pub fn workspace_env(&self) -> &BTreeMap<String, String> {
        &self.workspace_env
    }

    /// `WORKBENCH_NETWORK_FS`: "1" forces network, "0" forces local.
    #[must_use]
    pub fn workspace_network_override(&self) -> Option<&str> {
        self.workspace_network_override.as_deref()
    }

    /// `WORKBENCH_SESSION_ID`: the session identity a launcher named for a bridge.
    #[must_use]
    pub fn workspace_session_id(&self) -> Option<&str> {
        self.workspace_session_id.as_deref()
    }

    /// Read-only workspace configuration resolution over the captured env.
    #[must_use]
    pub fn workspace_resolve_options(&self) -> ResolveOptions {
        ResolveOptions {
            root: self.project_root.clone(),
            env: self.workspace_env.clone(),
            session_flags: Default::default(),
        }
    }

    /// Origins of every captured setting. Source names only — never values.
    #[must_use]
    pub fn sources(&self) -> &BTreeMap<&'static str, InputSource> {
        &self.sources
    }

    // ── Derived paths ────────────────────────────────────────────────

    /// `<config_dir>/config.toml`.
    #[must_use]
    pub fn user_config_path(&self) -> Option<PathBuf> {
        self.user_path(USER_CONFIG_FILE)
    }

    /// `<config_dir>/presets`.
    #[must_use]
    pub fn user_preset_dir(&self) -> Option<PathBuf> {
        self.user_path(USER_PRESET_DIR)
    }

    /// `<config_dir>/models.toml`.
    #[must_use]
    pub fn models_config_path(&self) -> Option<PathBuf> {
        self.user_path(MODELS_FILE)
    }

    /// `<config_dir>/plugins`.
    #[must_use]
    pub fn plugins_dir(&self) -> Option<PathBuf> {
        self.user_path(PLUGINS_DIR)
    }

    /// `<config_dir>/org-plugins.json`.
    #[must_use]
    pub fn org_plugin_config_path(&self) -> Option<PathBuf> {
        self.user_path(ORG_PLUGIN_CONFIG_FILE)
    }

    fn user_path(&self, leaf: &str) -> Option<PathBuf> {
        self.config_dir.as_ref().map(|dir| dir.join(leaf))
    }

    /// `<project_root>/.magistr/project.json`.
    #[must_use]
    pub fn project_json(&self) -> PathBuf {
        crate::paths::project_json(&self.project_root)
    }

    /// `<project_root>/.magistr/presets`.
    #[must_use]
    pub fn project_preset_dir(&self) -> PathBuf {
        self.project_root.join(PROJECT_PRESET_DIR)
    }

    // ── Loaders ──────────────────────────────────────────────────────

    /// The user's `config.toml`: loaded, absent (defaults) or unlocated
    /// (defaults). An existing file that cannot be read or parsed is an error.
    pub fn user_config(&self) -> std::result::Result<CapturedInput<UserConfig>, UserConfigError> {
        let Some(path) = self.user_config_path() else {
            return Ok(CapturedInput::unlocated(UserConfig::default()));
        };
        if !path.is_file() {
            return Ok(CapturedInput::absent(UserConfig::default(), path));
        }
        let value = user_config::load_from(&path)?;
        Ok(CapturedInput::loaded(value, path))
    }

    /// The project's `.magistr/project.json` as written, without the
    /// detection-driven rewrite `validate::load_project_config` performs.
    /// `None` when the file is absent; an unreadable or invalid file is an error.
    pub fn project_config(&self) -> Result<CapturedInput<Option<ProjectConfig>>> {
        let path = self.project_json();
        if !path.is_file() {
            return Ok(CapturedInput::absent(None, path));
        }
        let content = std::fs::read_to_string(&path).map_err(|e| MagistrError::io(&path, e))?;
        let config = serde_json::from_str(&content).map_err(|e| {
            MagistrError::Config(format!("invalid project config at {}: {e}", path.display()))
        })?;
        Ok(CapturedInput::loaded(Some(config), path))
    }

    /// The user's override for `lang`, when the preset directory is located
    /// and holds one. An unparseable override is an error.
    pub fn user_preset_override(&self, lang: Language) -> Result<Option<PresetOverride>> {
        match self.user_preset_dir() {
            Some(dir) => load_user_override_from_dir(&dir, lang),
            None => Ok(None),
        }
    }

    /// User-global preset files with metadata, for `preset list`.
    #[must_use]
    pub fn user_preset_info(&self) -> Vec<PresetInfo> {
        self.user_preset_dir()
            .filter(|dir| dir.is_dir())
            .map(|dir| list_presets_in_dir(&dir, "user-global"))
            .unwrap_or_default()
    }

    /// Layer options that apply the user's override from the captured preset
    /// directory. Project-local fragments and interview layers stay the
    /// caller's decision.
    #[must_use]
    pub fn preset_layers(&self) -> LayerOptions {
        LayerOptions {
            user_preset_dir: self.user_preset_dir().map(PathBuf::into_boxed_path),
            ..LayerOptions::default()
        }
    }
}

#[cfg(test)]
#[path = "project_inputs.tests.rs"]
mod tests;
