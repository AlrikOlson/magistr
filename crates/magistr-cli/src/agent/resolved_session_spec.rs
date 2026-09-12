//! Immutable host session setup shared by every production launch path.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use agent_client_protocol::schema::v1::McpServer;
use magistr_acp::{AcpSessionMode, AcpTransport};
use magistr_application::effects::SourceWriter;

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
    source_writer: Option<Arc<dyn SourceWriter>>,
}

impl ResolvedSessionSpec {
    pub(super) fn resolve(config: AgentConfig) -> Result<Self> {
        let defaults = crate::config::user_config::load().map_err(|error| {
            MagistrError::Config(format!("Cannot resolve session user defaults: {error}"))
        })?;
        Self::resolve_with_defaults(config, &defaults)
    }

    fn resolve_with_defaults(config: AgentConfig, defaults: &UserConfig) -> Result<Self> {
        validate_continuation(&config)?;
        let selected = config.provider.as_deref().or(defaults.session.provider());
        let runtime = AgentRuntime::detect_with_preference(selected)?;
        Self::resolve_with_runtime(config, defaults, runtime)
    }

    /// Injection seam for channel-backed launch tests; production resolves the
    /// runtime through the same provider selection before reaching this method.
    pub(crate) fn resolve_with_runtime(
        mut config: AgentConfig,
        defaults: &UserConfig,
        runtime: AgentRuntime,
    ) -> Result<Self> {
        use SessionConfigSource::{
            AutoDetected, Explicit, ProjectFile, ProjectPolicy, RuntimeDefault, RuntimeResume,
            UserDefault,
        };
        validate_continuation(&config)?;
        config.cwd = std::fs::canonicalize(&config.cwd)
            .map_err(|error| MagistrError::io(&config.cwd, error))?;
        let mut provenance = BTreeMap::new();
        let provider_source = match config.provider.as_deref() {
            Some("auto") => AutoDetected,
            Some(_) => Explicit,
            None if defaults.session.provider().is_some() => UserDefault,
            None => AutoDetected,
        };
        provenance.insert("provider", provider_source);
        config.provider = Some(runtime.provider.clone());
        let model_source = if config.model.is_some() {
            Explicit
        } else if config.resume.is_none()
            && let Some(model) = defaults.models.for_provider(runtime.provider_id())
        {
            config.model = Some(model.to_owned());
            UserDefault
        } else if config.resume.is_some() {
            RuntimeResume
        } else {
            RuntimeDefault
        };
        // "default" is a request to retain the provider's selected model.
        config.model = config
            .model
            .filter(|model| !model.eq_ignore_ascii_case("default"));
        let governance = crate::team::TeamConfig::load(&config.cwd)?
            .map(|team| team.policy)
            .unwrap_or_default();
        let model = resolve_model(&governance, config.model.as_deref())?;
        provenance.insert(
            "model",
            if model == config.model {
                model_source
            } else {
                ProjectPolicy
            },
        );
        config.model = model;
        provenance.insert(
            "effort",
            if config.effort.is_some() {
                Explicit
            } else if config.resume.is_some() {
                RuntimeResume
            } else {
                RuntimeDefault
            },
        );
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
            permission_config,
            governance,
            profile,
            config.workflow_permissions,
            config.model.as_deref(),
        )?;
        let mcp_servers = resolved_mcp_servers(&config.cwd)?;
        Ok(Self {
            config,
            runtime,
            mcp_servers,
            policy,
            handling,
            profile,
            provenance,
            source_writer: None,
        })
    }

    /// Route the session's authorized writes through the host's admitted
    /// workspace; `None` keeps the direct write and the effect says so.
    #[must_use]
    pub fn with_source_writer(mut self, writer: Option<Arc<dyn SourceWriter>>) -> Self {
        self.source_writer = writer;
        self
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
        .local_effects();
        if let Some(writer) = &self.source_writer {
            transport = transport.source_writer(Arc::clone(writer));
        }
        transport.connect().await.map_err(|error| {
            MagistrError::Acp(format!("Failed to connect ACP transport: {error}"))
        })?;
        Ok(transport)
    }

    /// A user-configured default model is a preference the agent may answer
    /// with its nearest offered model; an explicit or governance-routed model
    /// is a requirement the agent must apply exactly.
    fn session_model_choice(&self, preferred: bool) -> Option<String> {
        let from_user_default =
            self.provenance.get("model") == Some(&SessionConfigSource::UserDefault);
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
