use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use magistr_workspace::config::ResolveOptions;

use super::{CapturedInput, InputSource};
use crate::config::preset_full::LayerOptions;
use crate::config::project_preset::{PROJECT_PRESET_DIR, PresetInfo, list_presets_in_dir};
use crate::config::user_config::{self, UserConfig, UserConfigError};
use crate::config::user_preset::load_user_override_from_dir;
use crate::error::Result;
use crate::types::{Language, PresetOverride, ProjectConfig};

const XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
const XDG_DATA_HOME: &str = "XDG_DATA_HOME";
const HOME: &str = "HOME";
const USERPROFILE: &str = "USERPROFILE";
const LOCALAPPDATA: &str = "LOCALAPPDATA";
/// Overrides the whole per-user state directory, `GNUPGHOME`-style: the
/// variable is the directory itself, so tests and sandboxes never touch a
/// real home.
const MAGISTR_HOME: &str = "MAGISTR_HOME";
const CODEX_HOME: &str = "CODEX_HOME";
const MAGISTR_USER: &str = "MAGISTR_USER";
const USER: &str = "USER";
const USERNAME: &str = "USERNAME";
const CI: &str = "CI";
const MAGISTR_GOAL: &str = "MAGISTR_GOAL";
const MAGISTR_BUDGET: &str = "MAGISTR_BUDGET";
const MAGISTR_BUDGET_REQUESTED: &str = "MAGISTR_BUDGET_REQUESTED";
const MAGISTR_GITHUB_CLIENT_ID: &str = "MAGISTR_GITHUB_CLIENT_ID";
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
/// The per-user state directory under the home, `~/.magistr`.
const STATE_DIR: &str = ".magistr";
/// The Codex CLI's own home under the user's, `~/.codex`.
const CODEX_DIR: &str = ".codex";

/// Project, preset, provider, plugin, workspace, machine and session inputs
/// captured once at composition. Use cases resolve paths and load files from
/// this set; they never consult the process environment again.
///
/// Secrets are never captured: the Claude API key is recorded only as
/// present or absent.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectInputs {
    project_root: PathBuf,
    workflow: magistr_engine::NativeConfig,
    /// `$HOME`, else `$USERPROFILE`, else none.
    home_dir: Option<PathBuf>,
    /// `$XDG_CONFIG_HOME/magistr`, else `<home>/.config/magistr`, else none.
    config_dir: Option<PathBuf>,
    /// `$XDG_DATA_HOME/magistr`, else `<home>/.local/share/magistr`, else
    /// `$LOCALAPPDATA/magistr`, else none.
    data_dir: Option<PathBuf>,
    /// `$MAGISTR_HOME`, else `<home>/.magistr`, else none.
    state_dir: Option<PathBuf>,
    /// `$CODEX_HOME`, else `<home>/.codex`, else none.
    codex_home: Option<PathBuf>,
    /// `MAGISTR_USER`, the explicit identity override.
    user_override: Option<String>,
    /// `USER`, else `USERNAME`.
    os_user: Option<String>,
    /// `CI` set to a non-empty value other than `0` or `false`.
    ci: bool,
    /// `MAGISTR_GOAL`, unparsed.
    goal_mode: Option<String>,
    /// `MAGISTR_BUDGET`: the limit the session runs under, in dollars.
    budget_limit: Option<f64>,
    /// `MAGISTR_BUDGET_REQUESTED`: the developer's own limit, in dollars.
    budget_requested: Option<f64>,
    /// `MAGISTR_GITHUB_CLIENT_ID`, the GitHub OAuth app for `magistr login`.
    github_client_id: Option<String>,
    agent_backend: Option<String>,
    session_selection: Option<crate::config::session_selection::SessionSelection>,
    claude_api_key: bool,
    configured_api_providers: Vec<String>,
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

        // The home and everything that hangs off it. Each derived directory
        // records the variable that actually decided it: an explicit
        // override, or the home it was derived from.
        let home = value(HOME)
            .map(|home| (home, HOME))
            .or_else(|| value(USERPROFILE).map(|home| (home, USERPROFILE)));
        sources.insert(
            "user.home",
            home.as_ref().map_or(InputSource::Default, |(_, key)| {
                InputSource::Environment(key)
            }),
        );
        let home_dir = home.as_ref().map(|(home, _)| PathBuf::from(home));
        let under_home = |leaf: &[&str]| {
            home.as_ref().map(|(home, key)| {
                let mut path = PathBuf::from(home);
                path.extend(leaf);
                (path, *key)
            })
        };
        let resolved = |explicit: &'static str, leaf: &[&str], joined: bool| {
            value(explicit)
                .map(|dir| {
                    let mut path = PathBuf::from(dir);
                    if joined {
                        path.push(USER_DIR);
                    }
                    (path, explicit)
                })
                .or_else(|| under_home(leaf))
        };
        let mut capture_dir =
            |setting: &'static str, dir: Option<(PathBuf, &'static str)>| -> Option<PathBuf> {
                sources.insert(
                    setting,
                    dir.as_ref().map_or(InputSource::Default, |(_, key)| {
                        InputSource::Environment(key)
                    }),
                );
                dir.map(|(path, _)| path)
            };
        let config_dir = capture_dir(
            "user.config_dir",
            resolved(XDG_CONFIG_HOME, &[".config", USER_DIR], true),
        );
        let data_dir = capture_dir(
            "user.data_dir",
            resolved(XDG_DATA_HOME, &[".local", "share", USER_DIR], true).or_else(|| {
                value(LOCALAPPDATA).map(|dir| (PathBuf::from(dir).join(USER_DIR), LOCALAPPDATA))
            }),
        );
        let state_dir = capture_dir(
            "user.state_dir",
            resolved(MAGISTR_HOME, &[STATE_DIR], false),
        );
        let codex_home = capture_dir("codex.home", resolved(CODEX_HOME, &[CODEX_DIR], false));

        let user_override = value(MAGISTR_USER);
        sources.insert(
            "identity.user",
            Self::source_for(user_override.is_some(), MAGISTR_USER),
        );
        let os_user = value(USER)
            .map(|user| (user, USER))
            .or_else(|| value(USERNAME).map(|user| (user, USERNAME)));
        sources.insert(
            "identity.os_user",
            os_user.as_ref().map_or(InputSource::Default, |(_, key)| {
                InputSource::Environment(key)
            }),
        );
        let os_user = os_user.map(|(user, _)| user);
        let ci = value(CI).is_some_and(|v| v != "0" && !v.eq_ignore_ascii_case("false"));
        sources.insert("identity.ci", Self::source_for(ci, CI));

        let goal_mode = value(MAGISTR_GOAL);
        sources.insert(
            "session.goal_mode",
            Self::source_for(goal_mode.is_some(), MAGISTR_GOAL),
        );
        let budget_limit = value(MAGISTR_BUDGET).and_then(|v| Self::dollars(&v));
        sources.insert(
            "session.budget_limit",
            Self::source_for(budget_limit.is_some(), MAGISTR_BUDGET),
        );
        let budget_requested = value(MAGISTR_BUDGET_REQUESTED).and_then(|v| Self::dollars(&v));
        sources.insert(
            "session.budget_requested",
            Self::source_for(budget_requested.is_some(), MAGISTR_BUDGET_REQUESTED),
        );
        let github_client_id = value(MAGISTR_GITHUB_CLIENT_ID);
        sources.insert(
            "login.github_client_id",
            Self::source_for(github_client_id.is_some(), MAGISTR_GITHUB_CLIENT_ID),
        );

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
        let configured_api_providers = [
            ("openai", vec!["OPENAI_API_KEY"]),
            ("gemini", vec!["GEMINI_API_KEY", "GOOGLE_API_KEY"]),
            ("ollama", vec!["OLLAMA_HOST"]),
        ]
        .into_iter()
        .filter_map(|(provider, keys)| {
            keys.iter()
                .any(|key| value(key).is_some())
                .then(|| provider.to_owned())
        })
        .collect();

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

        let project_root = project_root.into();
        let workflow = magistr_engine::NativeConfig::from_lookup(&project_root, value);
        Self {
            project_root,
            workflow,
            home_dir,
            config_dir,
            data_dir,
            state_dir,
            codex_home,
            user_override,
            os_user,
            ci,
            goal_mode,
            budget_limit,
            budget_requested,
            github_client_id,
            agent_backend,
            session_selection: None,
            claude_api_key,
            configured_api_providers,
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

    /// A positive, finite dollar amount, or nothing: a budget that cannot
    /// cap anything is no budget.
    fn dollars(value: &str) -> Option<f64> {
        value
            .parse::<f64>()
            .ok()
            .filter(|dollars| dollars.is_finite() && *dollars > 0.0)
    }

    // ── Overrides ────────────────────────────────────────────────────

    /// The same captured user/provider/plugin/workspace inputs re-rooted at
    /// another project — what a `--dir` command uses.
    #[must_use]
    pub fn for_project(&self, root: impl Into<PathBuf>) -> Self {
        let mut inputs = self.clone();
        inputs.project_root = root.into();
        inputs.workflow = inputs.workflow.with_project_root(&inputs.project_root);
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

    /// Point the per-user data directory (the cost ledger, the sessions
    /// database, the audit log) somewhere explicit — what a test does
    /// instead of setting `XDG_DATA_HOME`.
    #[must_use]
    pub fn with_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.data_dir = Some(dir.into());
        self.sources.insert("user.data_dir", InputSource::Explicit);
        self
    }

    /// Point the per-user state directory (the verified-identity cache)
    /// somewhere explicit — what a test does instead of setting
    /// `MAGISTR_HOME`.
    #[must_use]
    pub fn with_state_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.state_dir = Some(dir.into());
        self.sources.insert("user.state_dir", InputSource::Explicit);
        self
    }

    /// Bind the budget the host decided: `limit` is what the session runs
    /// under after governance, `requested` the developer's own flag or
    /// user-config limit. Both replace whatever the environment exported —
    /// the decision is the host's, and `None` means nothing capped it.
    #[must_use]
    pub fn with_budget(mut self, limit: Option<f64>, requested: Option<f64>) -> Self {
        let positive = |dollars: Option<f64>| dollars.filter(|d| d.is_finite() && *d > 0.0);
        self.budget_limit = positive(limit);
        self.budget_requested = positive(requested);
        self.sources
            .insert("session.budget_limit", InputSource::Explicit);
        self.sources
            .insert("session.budget_requested", InputSource::Explicit);
        self
    }

    /// Freeze the same non-secret agent settings for every session in this run.
    #[must_use]
    pub fn with_session_selection(
        mut self,
        selection: crate::config::session_selection::SessionSelection,
    ) -> Self {
        self.session_selection = Some(selection);
        self
    }

    #[must_use]
    pub fn session_selection(&self) -> Option<&crate::config::session_selection::SessionSelection> {
        self.session_selection.as_ref()
    }

    /// Workflow storage and execution inputs captured with the session inputs.
    #[must_use]
    pub fn workflow(&self) -> &magistr_engine::NativeConfig {
        &self.workflow
    }

    // ── Captured values ──────────────────────────────────────────────

    /// The project root the inputs are bound to.
    #[must_use]
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// The user's home directory (`HOME`, else `USERPROFILE`), when set.
    #[must_use]
    pub fn home_dir(&self) -> Option<&Path> {
        self.home_dir.as_deref()
    }

    /// Magistr's per-user configuration directory, when one could be resolved.
    #[must_use]
    pub fn config_dir(&self) -> Option<&Path> {
        self.config_dir.as_deref()
    }

    /// Magistr's per-user data directory (`$XDG_DATA_HOME/magistr`, else
    /// `~/.local/share/magistr`, else `$LOCALAPPDATA/magistr`), when one
    /// could be resolved.
    #[must_use]
    pub fn data_dir(&self) -> Option<&Path> {
        self.data_dir.as_deref()
    }

    /// Magistr's per-user state directory (`$MAGISTR_HOME`, else
    /// `~/.magistr`), when one could be resolved.
    #[must_use]
    pub fn state_dir(&self) -> Option<&Path> {
        self.state_dir.as_deref()
    }

    /// The Codex CLI's home (`$CODEX_HOME`, else `~/.codex`), when one
    /// could be resolved.
    #[must_use]
    pub fn codex_home(&self) -> Option<&Path> {
        self.codex_home.as_deref()
    }

    /// `MAGISTR_USER`, the explicit identity override, trimmed and nonblank.
    #[must_use]
    pub fn user_override(&self) -> Option<&str> {
        self.user_override.as_deref()
    }

    /// The OS user name (`USER`, else `USERNAME`), trimmed and nonblank.
    #[must_use]
    pub fn os_user(&self) -> Option<&str> {
        self.os_user.as_deref()
    }

    /// Whether a CI environment was active at capture: `CI` set to a
    /// non-empty value other than `0` or `false`.
    #[must_use]
    pub fn is_ci(&self) -> bool {
        self.ci
    }

    /// `MAGISTR_GOAL` as written, trimmed and nonblank; the loop parses it.
    #[must_use]
    pub fn goal_mode(&self) -> Option<&str> {
        self.goal_mode.as_deref()
    }

    /// The limit the session runs under, in dollars: the governed budget
    /// the host bound, else the `MAGISTR_BUDGET` export it inherited.
    #[must_use]
    pub fn budget_limit(&self) -> Option<f64> {
        self.budget_limit
    }

    /// The developer's own limit, in dollars: the flag or user-config limit
    /// the host bound, else the `MAGISTR_BUDGET_REQUESTED` export it
    /// inherited.
    #[must_use]
    pub fn budget_requested(&self) -> Option<f64> {
        self.budget_requested
    }

    /// `MAGISTR_GITHUB_CLIENT_ID`, trimmed and nonblank.
    #[must_use]
    pub fn github_client_id(&self) -> Option<&str> {
        self.github_client_id.as_deref()
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

    /// API providers with credentials or a local endpoint configured at capture.
    /// Contains provider names only, never keys or endpoint values.
    #[must_use]
    pub fn configured_api_providers(&self) -> &[String] {
        &self.configured_api_providers
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

    /// The shared project configuration resolved from this working copy.
    pub fn project_json(&self) -> Result<PathBuf> {
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
        let document = user_config::UserConfigDocument::load(&path)?;
        let value = document.config().clone();
        Ok(if document.is_present() {
            CapturedInput::loaded(value, path)
        } else {
            CapturedInput::absent(value, path)
        })
    }

    /// The current shared project settings, read without changing them.
    /// `None` when the file is absent; an unreadable or invalid file is an error.
    pub fn project_config(&self) -> Result<CapturedInput<Option<ProjectConfig>>> {
        let path = self.project_json()?;
        match crate::config::read_project_settings(&path)? {
            Some(config) => Ok(CapturedInput::loaded(Some(config), path)),
            None => Ok(CapturedInput::absent(None, path)),
        }
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
