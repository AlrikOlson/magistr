//! The Claude provider.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use agent_client_protocol::BoxFuture;
use agent_client_protocol::schema::v1::{
    AvailableCommandsUpdate, SessionId, SessionUpdate, StopReason,
};

use crate::claude::control::ControlRequest;
use crate::claude::handshake::Handshake;
use crate::claude::launch::LaunchSpec;
use crate::claude::process::ClaudeCli;
use crate::claude::session::ClaudeSession;
use crate::claude::transcript;
use crate::{AgentError, ModelChoice, Provider, SessionOpened, SessionStart, Turn};

/// Drives the `claude` command over its stream-json control protocol (see
/// the [module docs](crate::claude)). One process per session; the model
/// catalog and the slash commands are read once, at
/// [`discover`](Self::discover), from the CLI's own initialize handshake.
pub struct ClaudeProvider {
    program: PathBuf,
    prefix: Vec<String>,
    config_dir: PathBuf,
    allowed_tools: Vec<String>,
    handshake: Handshake,
    sessions: Mutex<HashMap<SessionId, Arc<ClaudeSession>>>,
}

impl ClaudeProvider {
    /// The provider's name on the command line.
    pub const NAME: &'static str = "claude";

    /// Run the initialize handshake against `program` once and keep what it
    /// answered. Fails when the command cannot be spawned or refuses the
    /// handshake (the message carries the CLI's stderr).
    pub async fn discover(program: impl Into<PathBuf>) -> Result<Self, AgentError> {
        Self::discover_with(program, Vec::new()).await
    }

    /// [`discover`](Self::discover) with arguments placed before the
    /// protocol flags — what a test's fake `claude` takes its script from.
    pub async fn discover_with(
        program: impl Into<PathBuf>,
        prefix: Vec<String>,
    ) -> Result<Self, AgentError> {
        let program = program.into();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut args = prefix.clone();
        args.extend(LaunchSpec::BASE_ARGS.iter().map(|s| (*s).to_owned()));
        args.push("--no-session-persistence".to_owned());
        let cli = ClaudeCli::spawn(&program, &args, &cwd)?;
        let response = cli.control(ControlRequest::Initialize).await?;
        cli.close().await;
        if let Some(error) = response.error {
            return Err(AgentError::provider(format!(
                "claude refused initialize: {error}"
            )));
        }
        let handshake = Handshake::parse(&response.payload);
        if handshake.models.is_empty() {
            return Err(AgentError::provider(
                "claude answered initialize without a model catalog — is it logged in?",
            ));
        }
        Ok(Self {
            program,
            prefix,
            config_dir: transcript::default_config_dir(),
            allowed_tools: Vec::new(),
            handshake,
            sessions: Mutex::default(),
        })
    }

    /// Where Claude Code keeps its transcripts, for `session/load` replay;
    /// `$CLAUDE_CONFIG_DIR` or `~/.claude` unless set here.
    #[must_use]
    pub fn config_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config_dir = dir.into();
        self
    }

    /// Tools every session may run without asking, in the CLI's rule syntax
    /// (`Bash`, `Bash(git *)`, `Edit`, …) — passed as `--allowedTools`.
    #[must_use]
    pub fn allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }

    fn session(&self, id: &SessionId) -> Result<Arc<ClaudeSession>, AgentError> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()
            .ok_or_else(|| AgentError::UnknownSession(id.to_string()))
    }

    fn commands_update(&self) -> SessionUpdate {
        SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(
            self.handshake.commands.clone(),
        ))
    }

    async fn open(&self, start: SessionStart) -> Result<SessionOpened, AgentError> {
        start.validate(self)?;
        let mut replay = vec![self.commands_update()];
        if start.resume {
            replay.extend(transcript::replay(
                &self.config_dir,
                &start.cwd,
                &start.session_id.to_string(),
            )?);
        }
        let spec = LaunchSpec {
            cwd: start.cwd.clone(),
            session_id: start.session_id.to_string(),
            resume: start.resume,
            model: start.model.clone().or_else(|| {
                (!start.resume)
                    .then(|| self.handshake.models.first().map(|m| m.id.clone()))
                    .flatten()
            }),
            mcp_servers: start.mcp_servers.clone(),
            allowed_tools: self.allowed_tools.clone(),
        };
        let session = ClaudeSession::open(&self.program, &self.prefix, &spec).await?;
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(start.session_id.clone(), Arc::new(session));
        let mut opened = SessionOpened::new(start.session_id).replay(replay);
        opened.model = spec.model;
        Ok(opened)
    }
}

impl Provider for ClaudeProvider {
    fn capabilities(&self) -> crate::ProviderCapabilities {
        crate::ProviderCapabilities {
            durable_load: true,
            history_replay: true,
            tools: true,
            ..crate::ProviderCapabilities::text_session()
        }
    }
    fn name(&self) -> &str {
        Self::NAME
    }

    fn models(&self) -> Vec<ModelChoice> {
        self.handshake.models.clone()
    }

    fn start(&self, session: SessionStart) -> BoxFuture<'_, Result<SessionOpened, AgentError>> {
        Box::pin(self.open(session))
    }

    fn prompt(&self, turn: Turn) -> BoxFuture<'_, Result<StopReason, AgentError>> {
        Box::pin(async move {
            let session = self.session(turn.session_id())?;
            session.run_turn(&turn).await
        })
    }
}

impl Drop for ClaudeProvider {
    fn drop(&mut self) {
        // The processes die with their handles (kill_on_drop); nothing to
        // wait for here.
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}
