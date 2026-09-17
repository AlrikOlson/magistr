// ── Agent-internal modules ──────────────────────────────────────────────

pub mod auth_method;
pub mod backend;
pub mod cockpit_session;
pub mod connect_transport;
pub mod convert_acp_event;
pub mod error_kind;
pub mod init_prompt;
pub mod mcp_servers;
pub mod message;
#[cfg(test)]
pub mod mock;
#[cfg(test)]
pub mod mock_followups;
pub mod observation;
pub mod permission_request;
pub mod probe;
pub mod probe_model;
pub mod provider_model_set;
pub mod resolved_session_spec;
pub mod retry;
pub mod retry_backoff;
pub mod retry_outcome;
pub mod runtime;
pub mod session;
pub mod session_config_source;
pub mod session_supervisor;
pub mod system_prompt;
pub mod tool_permissions;
pub mod translate;
pub mod types;

// ── Consolidated sub-domains ────────────────────────────────────────────

pub mod engine;
pub mod models;
pub mod multi;
pub mod noninteractive_response;
pub mod permission_mode;
pub mod persistence;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::inputs::ProjectInputs;

pub use auth_method::AuthMethod;
pub use connect_transport::connect_transport;
pub use permission_mode::PermissionMode;
pub use resolved_session_spec::ResolvedSessionSpec;
pub use runtime::AgentRuntime;
pub use session_config_source::SessionConfigSource;
pub use tool_permissions::ToolPermissions;

/// Translate canonical tool names to the format expected by a specific runtime.
///
/// Different backends use different `--allow-tool` syntax:
/// - **Claude Code** (the in-binary `claude` provider): PascalCase names (`Bash`, `Read`, `Write`)
/// - **Copilot CLI**: lowercase with patterns (`shell(*)`, `read`, `write`)
///
/// MCP tool names (like `roadmap_*` or `magistr_gate`) are passed through as-is since MCP
/// naming is consistent across backends.
pub fn translate_tools_for_runtime(tools: &[String], auth: &AuthMethod) -> Vec<String> {
    match auth {
        AuthMethod::Copilot => tools
            .iter()
            .map(|t| translate_tool_for_copilot(t))
            .collect(),
        _ => tools.to_vec(),
    }
}

/// Map a single canonical tool name to Copilot CLI format.
fn translate_tool_for_copilot(tool: &str) -> String {
    match tool {
        "Bash" => "shell(*)".to_string(),
        "Read" => "read".to_string(),
        "Write" => "write".to_string(),
        "Edit" => "edit".to_string(),
        "Glob" => "read".to_string(), // Copilot bundles glob under read
        "Grep" => "read".to_string(), // Copilot bundles grep under read
        "WebSearch" | "web_search" => "web_search".to_string(),
        "WebFetch" => "web_fetch".to_string(),
        "NotebookEdit" => "notebook_edit".to_string(),
        "TodoRead" => "todo_read".to_string(),
        "TodoWrite" => "todo_write".to_string(),
        // MCP tools pass through as-is
        other => other.to_string(),
    }
}

// ── Backend ──────────────────────────────────────────────────────────────

// Canonical definitions live in magistr-core. Re-export for backward compatibility.
pub use magistr_core::{Backend, BackendCapabilities};

/// Detect the best available backend.
///
/// Checks for the agent binaries (`claude` or `copilot`)
/// on PATH. Returns `None` if no backend is available.
#[must_use]
pub fn detect_backend() -> Option<Backend> {
    if is_backend_available(Backend::Acp) {
        return Some(Backend::Acp);
    }
    None
}

/// Check whether a backend is available on the system.
#[must_use]
pub fn is_backend_available(backend: Backend) -> bool {
    match backend {
        Backend::Acp => AgentRuntime::is_any_binary_available(),
    }
}

/// Configuration for a magistr agent session.
///
/// Magistr-specific config (project dir, system prompt, model, permissions)
/// that `AcpBackend::start` turns into an agent launch and a session.
#[derive(Clone)]
pub struct AgentConfig {
    /// The prompt to send to Claude
    pub prompt: String,
    /// Working directory for the agent session
    pub cwd: PathBuf,
    /// System prompt injected with project context
    pub system_prompt: Option<String>,
    /// Maximum turns before stopping
    pub max_turns: Option<u32>,
    /// Legacy provider permission mode. Explicit use is refused; shared
    /// permission profiles remain the common authority.
    pub permission_mode: Option<PermissionMode>,
    /// Project permission profile override, normally selected by `--trust`.
    pub permission_profile: Option<crate::permissions::PermissionProfile>,
    /// Whether the shared policy should admit normal workflow operations.
    pub workflow_permissions: bool,
    /// Restrict this session to native planning and read-only source context.
    pub planning_only: bool,
    /// Whether to continue from the previous conversation
    pub continue_conversation: bool,
    /// Session ID to resume from
    pub resume: Option<String>,
    /// Explicit runtime provider for this session. None uses the user's
    /// default; "auto" requests automatic detection even with a global default.
    pub provider: Option<String>,
    /// Model to use (e.g., "sonnet", "opus", "claude-sonnet-4-20250514")
    pub model: Option<String>,
    /// Explicit provider reasoning effort; unsupported selections fail before inference.
    pub effort: Option<String>,
    /// Which agent backend to use
    pub backend: Backend,
    /// Tools pre-approved for automatic use (passed as `--allow-tool` flags).
    ///
    /// Populated from [`ToolPermissions::base()`] by default; callers can
    /// extend via [`allowed_tools`](Self::allowed_tools) or
    /// [`allow_tool`](Self::allow_tool).
    pub allowed_tools: Vec<String>,
    /// The project, provider and user inputs captured when this configuration
    /// was composed. Resolution reads only these; it never consults the
    /// process environment again.
    inputs: Arc<ProjectInputs>,
}

impl AgentConfig {
    /// Create a new `AgentConfig` for a prompt in the given project directory.
    ///
    /// Composition captures the project inputs once here; a host that already
    /// captured them at startup hands its set over with [`Self::with_inputs`].
    /// Permission profiles and the host policy are the only enforced authority.
    /// The legacy provider mode remains unset unless a caller explicitly asks
    /// for it, in which case resolution refuses before provider startup.
    pub fn new(prompt: impl Into<String>, cwd: impl AsRef<Path>) -> Self {
        Self {
            prompt: prompt.into(),
            inputs: Arc::new(ProjectInputs::from_env(cwd.as_ref())),
            cwd: cwd.as_ref().to_path_buf(),
            system_prompt: None,
            max_turns: None,
            permission_mode: None,
            permission_profile: None,
            workflow_permissions: false,
            planning_only: false,
            continue_conversation: false,
            resume: None,
            provider: None,
            model: None,
            effort: None,
            backend: Backend::default(),
            allowed_tools: ToolPermissions::base().into_strings(),
        }
    }

    /// Set the system prompt.
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Auto-inject system prompt from project config if available.
    ///
    /// Loads `.magistr/project.json` and `.claude/rules/*.md` from the given
    /// directory and sets the system prompt. No-op if no config is found.
    /// Consolidates the repeated `if let Some(sp) = build_system_prompt(...)`
    /// pattern used across command handlers.
    pub fn with_project_context(self, project_dir: &Path) -> Self {
        if let Some(sp) = system_prompt::build_system_prompt(project_dir) {
            self.system_prompt(sp)
        } else {
            self
        }
    }

    /// Set the maximum number of turns.
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.max_turns = Some(turns);
        self
    }

    /// Request the legacy provider permission mode.
    ///
    /// Resolution rejects this request before provider startup because common
    /// host permission policy remains authoritative.
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = Some(mode);
        self
    }

    /// Override the selected project's permission profile.
    pub fn permission_profile(mut self, profile: crate::permissions::PermissionProfile) -> Self {
        self.permission_profile = Some(profile);
        self
    }

    /// Apply the shared workflow policy to this session.
    pub fn workflow_permissions(mut self, workflow: bool) -> Self {
        self.workflow_permissions = workflow;
        self
    }

    /// Load the shared project policy and its noninteractive response setting.
    /// Uses this session's root even when the process is in another project.
    pub fn session_permission_policy(
        &self,
    ) -> (
        crate::permissions::SessionPermissionPolicy,
        crate::permissions::NoninteractivePolicy,
    ) {
        let config = crate::permissions::load_config(&self.cwd).unwrap_or_default();
        let profile = self.permission_profile.unwrap_or(config.profile);
        let policy = crate::orchestration::commands::run::cockpit_policy_for_project(
            &self.cwd,
            profile,
            self.workflow_permissions,
            self.model.as_deref(),
        );
        (policy, config.noninteractive)
    }

    /// Continue from the previous conversation.
    pub fn continue_conversation(mut self, cont: bool) -> Self {
        self.continue_conversation = cont;
        self
    }

    /// Resume a previous session by ID.
    pub fn resume(mut self, session_id: impl Into<String>) -> Self {
        self.resume = Some(session_id.into());
        self
    }

    /// Select the runtime provider for this session without changing global
    /// preferences. Unknown IDs fail at launch; they never select a fallback.
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Use the host's captured inputs instead of the set captured at
    /// construction, so a session resolves against the same capture as the
    /// command that launched it.
    #[must_use]
    pub fn with_inputs(mut self, inputs: &ProjectInputs) -> Self {
        self.inputs = Arc::new(inputs.clone());
        self
    }

    /// The captured inputs this session resolves against.
    #[must_use]
    pub fn inputs(&self) -> &Arc<ProjectInputs> {
        &self.inputs
    }

    /// Resolve this session's provider, using the captured user defaults only
    /// when no session override was supplied.
    ///
    /// # Errors
    ///
    /// The provider is unsupported or its runtime cannot be found.
    pub fn resolve_runtime(&self) -> crate::error::Result<AgentRuntime> {
        let preferred = match self.provider.as_deref() {
            Some(provider) => Some(provider.to_owned()),
            None => self
                .inputs
                .user_config()
                .ok()
                .and_then(|cfg| cfg.value.session.provider().map(str::to_owned)),
        };
        AgentRuntime::detect_for(&self.inputs, preferred.as_deref())
    }

    /// Set the model to use for this session.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Select reasoning effort for providers that support it.
    pub fn effort(mut self, effort: impl Into<String>) -> Self {
        self.effort = Some(effort.into());
        self
    }

    /// Freeze all session choices before connecting a runtime.
    ///
    /// # Errors
    ///
    /// Unsupported continuation, invalid provider, unavailable runtime, or refused model.
    pub fn resolve(self) -> crate::error::Result<ResolvedSessionSpec> {
        ResolvedSessionSpec::resolve(self)
    }

    /// Set the agent backend to use for this session.
    pub fn backend(mut self, backend: Backend) -> Self {
        self.backend = backend;
        self
    }

    /// Apply common CLI run arguments in one call.
    ///
    /// Consolidates the repeated pattern of conditionally setting model,
    /// resume, and continue flags from optional CLI args. Equivalent to
    /// chaining `.model()`, `.resume()`, and `.continue_conversation()`
    /// individually but avoids duplicating the `if let Some` boilerplate.
    pub fn with_run_args(
        mut self,
        model: Option<&str>,
        resume: Option<&str>,
        continue_conversation: bool,
    ) -> Self {
        if let Some(m) = model {
            self.model = Some(m.into());
        }
        if let Some(sid) = resume {
            self.resume = Some(sid.into());
        }
        if continue_conversation {
            self.continue_conversation = true;
        }
        self
    }

    /// Replace the entire allowed-tools list.
    pub fn allowed_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_tools = tools.into_iter().map(Into::into).collect();
        self
    }

    /// Add a single tool to the pre-approved list.
    pub fn allow_tool(mut self, tool: impl Into<String>) -> Self {
        let tool = tool.into();
        if !self.allowed_tools.contains(&tool) {
            self.allowed_tools.push(tool);
        }
        self
    }

    /// Apply user-configured defaults to this config.
    ///
    /// Loads the captured user config, pins the selected provider and
    /// applies only that provider's model default to new sessions. Resumed
    /// sessions retain the runtime model unless the caller supplied an override.
    /// Compatibility builder for callers inspecting unresolved configuration.
    /// Production startup applies defaults while creating [`ResolvedSessionSpec`].
    ///
    /// Config loading or automatic detection failures are left for launch to
    /// report. Explicit unsupported providers are preserved for launch refusal.
    #[must_use]
    pub fn apply_user_defaults(self) -> Self {
        let inputs = Arc::clone(&self.inputs);
        match inputs.user_config() {
            Ok(defaults) => self.apply_defaults_from(&defaults.value),
            Err(_) => self,
        }
    }

    fn apply_defaults_from(mut self, defaults: &crate::config::user_config::UserConfig) -> Self {
        if self.provider.is_none() {
            self.provider = defaults.session.provider().map(str::to_owned);
        }
        if self
            .provider
            .as_deref()
            .is_none_or(|provider| provider == "auto")
            && let Ok(runtime) = AgentRuntime::detect_for(&self.inputs, None)
        {
            self.provider = Some(runtime.provider);
        }
        if self.model.is_none()
            && self.resume.is_none()
            && let Some(provider) = self.provider.as_deref()
            && let Some(model) = defaults.models.for_provider(provider)
        {
            self.model = Some(model.to_owned());
        }
        self
    }
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
