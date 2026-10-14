//! Immutable host session setup shared by every production launch path.

use std::collections::BTreeMap;
use std::path::Path;

use agent_client_protocol::schema::v1::McpServer;
use magistr_acp::{AcpSessionMode, AcpTransport};
use magistr_application::effects::{SourceRefusal, SourceRoute};

use super::{AgentConfig, AgentRuntime, SessionConfigSource};
use crate::config::user_config::UserConfig;
use crate::error::{MagistrError, Result};
use crate::permissions::{NoninteractivePolicy, PermissionProfile, SessionPermissionPolicy};

/// Frozen session inputs and policy. Accessors borrow choices; consuming this
/// value transfers its policy to the session owner. Global configuration and
/// project MCP files are never consulted by transport startup afterwards.
pub struct ResolvedSessionSpec {
    config: AgentConfig,
    runtime: AgentRuntime,
    mcp_servers: Vec<McpServer>,
    policy: SessionPermissionPolicy,
    handling: NoninteractivePolicy,
    profile: PermissionProfile,
    provenance: BTreeMap<&'static str, SessionConfigSource>,
    source: SourceRoute,
}

impl ResolvedSessionSpec {
    /// Freeze the session from the inputs captured when `config` was
    /// composed: the user defaults come from the captured config path and
    /// the runtime is detected against the captured provider facts.
    pub(super) fn resolve(config: AgentConfig) -> Result<Self> {
        let defaults = if config.inputs.session_selection().is_some() {
            UserConfig::default()
        } else {
            config
                .inputs
                .user_config()
                .map_err(|error| {
                    MagistrError::Config(format!("Cannot resolve session user defaults: {error}"))
                })?
                .value
        };
        Self::resolve_with_defaults(config, &defaults)
    }

    fn resolve_with_defaults(config: AgentConfig, defaults: &UserConfig) -> Result<Self> {
        validate_continuation(&config)?;
        let project = if config.inputs.session_selection().is_some() {
            crate::config::agent_preferences::AgentPreferences::default()
        } else {
            crate::config::agent_preferences::AgentPreferences::load(&config.cwd)?
        };
        let selected = config
            .provider
            .as_deref()
            .or_else(|| {
                config
                    .inputs
                    .session_selection()
                    .map(|selection| selection.agent.as_str())
            })
            .or(project.agent.as_deref())
            .or(defaults.session.provider());
        let runtime = AgentRuntime::detect_for(&config.inputs, selected)?;
        Self::resolve_with_runtime(config, defaults, runtime)
    }

    /// Injection seam for channel-backed launch tests; production resolves the
    /// runtime through the same provider selection before reaching this method.
    pub(crate) fn resolve_with_runtime(
        mut config: AgentConfig,
        defaults: &UserConfig,
        runtime: AgentRuntime,
    ) -> Result<Self> {
        use SessionConfigSource::{Explicit, ProjectFile, ProjectPolicy, RuntimeDefault};
        validate_continuation(&config)?;
        config.cwd = std::fs::canonicalize(&config.cwd)
            .map_err(|error| MagistrError::io(&config.cwd, error))?;
        let mut provenance =
            super::session_defaults::session_defaults(&mut config, defaults, &runtime)?;
        let governance = crate::team::TeamConfig::load(&config.cwd)?
            .map(|team| team.policy)
            .unwrap_or_default();
        let model = resolve_model(&governance, config.model.as_deref())?;
        if model != config.model
            && config.inputs.session_selection().is_some_and(|selection| {
                selection.agent == runtime.provider && selection.model == config.model
            })
        {
            return Err(MagistrError::Config(
                "This run's selected model is no longer allowed by the project's settings. Restore those settings or choose a different model for a new run after this one has ended".into(),
            ));
        }
        provenance.insert(
            "model",
            if model == config.model {
                provenance["model"]
            } else {
                ProjectPolicy
            },
        );
        config.model = model;
        provenance.insert("project_root", Explicit);
        provenance.insert(
            "resume",
            if config.resume.is_some() {
                Explicit
            } else {
                RuntimeDefault
            },
        );
        provenance.insert("mcp_servers", ProjectFile);
        provenance.insert(
            "permission_profile",
            if config.permission_profile.is_some() {
                Explicit
            } else {
                ProjectPolicy
            },
        );
        let permission_config = crate::permissions::load_config(&config.cwd)?;
        let profile = config
            .permission_profile
            .unwrap_or(permission_config.profile);
        let handling = permission_config.noninteractive;
        let policy = SessionPermissionPolicy::for_project_snapshot(
            &config.cwd,
            config.inputs(),
            permission_config,
            governance,
            profile,
            config.workflow_permissions,
            config.model.as_deref(),
        )?;
        let mut mcp_servers = resolved_mcp_servers(&config.cwd)?;
        if let Some(execution) = &config.execution_id {
            for server in &mut mcp_servers {
                if let McpServer::Stdio(server) = server {
                    server.env.retain(|var| {
                        var.name != "MAGISTR_EXECUTION_ID" && var.name != "WORKBENCH_SESSION_ID"
                    });
                    server
                        .env
                        .push(agent_client_protocol::schema::v1::EnvVariable::new(
                            "MAGISTR_EXECUTION_ID",
                            execution,
                        ));
                    server
                        .env
                        .push(agent_client_protocol::schema::v1::EnvVariable::new(
                            "WORKBENCH_SESSION_ID",
                            execution,
                        ));
                }
            }
            provenance.insert("execution_context", Explicit);
        }
        Ok(Self {
            config,
            runtime,
            mcp_servers,
            policy,
            handling,
            profile,
            provenance,
            source: SourceRoute::Refused(SourceRefusal::NoAuthority),
        })
    }

    /// The route the session's authorized source writes take: the host's
    /// admitted workspace, or a refusal. A spec resolved without one holds
    /// no source-write authority.
    #[must_use]
    pub fn with_source_route(mut self, source: SourceRoute) -> Self {
        self.source = if self.config.planning_only {
            SourceRoute::Refused(SourceRefusal::NoAuthority)
        } else {
            source
        };
        self
    }

    pub(crate) fn history(&self) -> super::persistence::SessionHistory {
        self.config.history.clone()
    }

    /// Project root supplied to transport, MCP discovery, and permission policy.
    #[must_use]
    pub fn cwd(&self) -> &Path {
        &self.config.cwd
    }

    /// Opening prompt retained independently from provider configuration.
    #[must_use]
    pub fn prompt(&self) -> &str {
        &self.config.prompt
    }

    /// Selected provider and executable, frozen before connection.
    #[must_use]
    pub fn runtime(&self) -> &AgentRuntime {
        &self.runtime
    }

    /// Explicit, defaulted or governance-routed model; absent preserves runtime state.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.config.model.as_deref()
    }

    /// Requested effort; support is checked by the selected provider during setup.
    #[must_use]
    pub fn effort(&self) -> Option<&str> {
        self.config.effort.as_deref()
    }

    /// Host-owned maximum number of turns, if configured.
    #[must_use]
    pub fn max_turns(&self) -> Option<u32> {
        self.config.max_turns
    }

    /// Runtime-owned resume selection.
    #[must_use]
    pub fn session_mode(&self) -> AcpSessionMode {
        self.config
            .resume
            .as_ref()
            .map_or(AcpSessionMode::New, |id| AcpSessionMode::Load {
                session_id: id.clone(),
            })
    }

    /// Selected project MCP snapshot. This may contain private environment values.
    #[must_use]
    pub fn mcp_servers(&self) -> &[McpServer] {
        &self.mcp_servers
    }

    /// The frozen permission profile selected for this session.
    #[must_use]
    pub fn permission_profile(&self) -> PermissionProfile {
        self.profile
    }

    /// Whether the session admits normal workflow operations.
    #[must_use]
    pub fn workflow_permissions(&self) -> bool {
        self.config.workflow_permissions
    }

    /// The non-secret settings that a workflow freezes and checkpoints.
    #[must_use]
    pub fn selection(&self) -> crate::config::session_selection::SessionSelection {
        crate::config::session_selection::SessionSelection {
            agent: self.runtime.provider.clone(),
            model: self.config.model.clone(),
            effort: self.config.effort.clone(),
            sources: self
                .provenance
                .iter()
                .map(|(key, source)| ((*key).to_string(), source.label().to_string()))
                .collect(),
        }
    }

    /// Non-secret evidence identifying which authority supplied each choice.
    #[must_use]
    pub fn provenance(&self) -> &BTreeMap<&'static str, SessionConfigSource> {
        &self.provenance
    }

    /// Spawn exactly this snapshot and complete validated setup before admitting input.
    ///
    /// # Errors
    ///
    /// Process startup, unsupported session choices, or ACP handshake failure.
    pub async fn connect_transport(&self) -> Result<AcpTransport> {
        self.connect_transport_checkpointed(|| Ok(())).await
    }

    /// The transport persists launch intent before creating the provider.
    pub(crate) async fn connect_transport_checkpointed(
        &self,
        before_spawn: impl FnOnce() -> Result<()>,
    ) -> Result<AcpTransport> {
        self.config
            .history
            .begin(
                self.config.inputs(),
                self.cwd(),
                self.runtime.provider_id(),
                self.model(),
            )
            .await?;
        let translated =
            super::translate_tools_for_runtime(&self.config.allowed_tools, &self.runtime.auth);
        let mut args = self.runtime.args.clone();
        let mut seen = std::collections::HashSet::new();
        for tool in translated {
            if seen.insert(tool.clone()) {
                args.push(format!("--allow-tool={tool}"));
            }
        }
        let mut transport = AcpTransport::new_with_args(
            self.runtime.path.clone(),
            args,
            Some(self.cwd().to_path_buf()),
        )
        .map_err(|error| MagistrError::Acp(format!("Failed to create ACP transport: {error}")))?
        .session_mode(self.session_mode())
        .mcp_servers(self.mcp_servers.clone())
        .session_model(self.session_model_choice(false))
        .session_model_preferred(self.session_model_choice(true))
        .session_effort(self.config.effort.clone())
        .session_system_prompt(self.config.system_prompt.clone())
        .local_effects(self.source.clone())
        .execution_id(self.config.execution_id.clone());
        if let Err(error) = transport
            .connect_checkpointed(|| {
                before_spawn().map_err(|error| magistr_acp::AcpError::connection(error.to_string()))
            })
            .await
        {
            // Failed setup has already settled its process. Saving history never
            // replaces that startup outcome.
            if let Err(save_error) = self
                .config
                .history
                .settle(super::persistence::session_outcome::SessionOutcome::Failed)
                .await
            {
                super::persistence::warn_history(&save_error);
            }
            return Err(MagistrError::Acp(format!(
                "Could not connect to the agent: {error}"
            )));
        }
        Ok(transport)
    }

    /// A user-configured default model is a preference the agent may answer
    /// with its nearest offered model; an explicit or governance-routed model
    /// is a requirement the agent must apply exactly.
    fn session_model_choice(&self, preferred: bool) -> Option<String> {
        let from_user_default = self.config.inputs.session_selection().is_none()
            && self.provenance.get("model") == Some(&SessionConfigSource::UserDefault);
        (from_user_default == preferred)
            .then(|| self.config.model.clone())
            .flatten()
    }

    /// Transfer the already resolved policy and unattended response mode to its owner.
    #[must_use]
    pub fn into_permission_policy(self) -> (SessionPermissionPolicy, NoninteractivePolicy) {
        (self.policy, self.handling)
    }
}

fn validate_continuation(config: &AgentConfig) -> Result<()> {
    if config.continue_conversation && config.resume.is_none() {
        return Err(MagistrError::Acp("Continuing the last conversation is unsupported; supply an explicit --resume session ID".into()));
    }
    if config.max_turns == Some(0) {
        return Err(MagistrError::Config(
            "max_turns must be greater than zero; the host refuses a zero-turn session before provider startup"
                .into(),
        ));
    }
    if config.permission_mode.is_some() {
        return Err(MagistrError::Config(
            "permission_mode is unsupported; use the shared permission profile and host policy"
                .into(),
        ));
    }
    Ok(())
}

fn resolve_model(
    governance: &magistr_engine::policy::GovernancePolicy,
    requested: Option<&str>,
) -> Result<Option<String>> {
    let Some(requested) = requested else {
        if !governance.approved_models.is_empty()
            || governance.max_model_tier.is_some()
            || governance
                .rules
                .iter()
                .any(|rule| references_model(&rule.condition))
        {
            return Err(MagistrError::Config("This project's model governance requires an explicit approved model; an unspecified runtime or resumed model cannot be verified before startup".into()));
        }
        return Ok(None);
    };
    governance.route_model(requested).map(|model| Some(model.to_owned())).ok_or_else(||
        MagistrError::Config(format!("model '{requested}' is not approved by this team's policy (.magistr/team.json) and no usable fallback is configured"))
    )
}

fn references_model(condition: &magistr_engine::policy::PolicyCondition) -> bool {
    use magistr_engine::policy::PolicyCondition;
    match condition {
        PolicyCondition::Model { .. } => true,
        PolicyCondition::AnyOf { conditions } | PolicyCondition::AllOf { conditions } => {
            conditions.iter().any(references_model)
        }
        PolicyCondition::Not { condition } => references_model(condition),
        _ => false,
    }
}

// Unlike discovery's best-effort reader, an executable session must not
// silently discard malformed or unsupported explicitly configured servers.
fn resolved_mcp_servers(root: &Path) -> Result<Vec<McpServer>> {
    let path = root.join(".mcp.json");
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(MagistrError::io(&path, error)),
    };
    let invalid = |detail: &str| {
        MagistrError::Config(format!(
            "Invalid session MCP configuration {}: {detail}",
            path.display()
        ))
    };
    let doc: serde_json::Value =
        serde_json::from_str(&raw).map_err(|error| invalid(&error.to_string()))?;
    let servers = doc
        .get("mcpServers")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| invalid("mcpServers must be an object"))?;
    for (name, server) in servers {
        let valid_command = server
            .get("command")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|command| !command.trim().is_empty());
        let supported_transport = server
            .get("type")
            .is_none_or(|kind| kind.as_str() == Some("stdio"))
            && server.get("url").is_none();
        let valid_args = server.get("args").is_none_or(|args| {
            args.as_array()
                .is_some_and(|args| args.iter().all(serde_json::Value::is_string))
        });
        let valid_env = server.get("env").is_none_or(|env| {
            env.as_object()
                .is_some_and(|env| env.values().all(serde_json::Value::is_string))
        });
        if !valid_command || !supported_transport || !valid_args || !valid_env {
            return Err(invalid(&format!(
                "server {name:?} requires supported stdio transport, a command, string arguments and string environment values"
            )));
        }
    }
    Ok(super::mcp_servers::mcp_servers_from_value(&doc))
}

#[cfg(test)]
#[path = "resolved_session_spec.tests.rs"]
mod tests;
