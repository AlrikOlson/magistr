//! The Codex subscription provider, backed by one local app-server.

use super::handshake::Handshake;
use super::process::CodexProcess;
use super::thread::CodexThread;
use super::thread_record::ThreadRecord;
use super::thread_spec::ThreadSpec;
use crate::{AgentError, ModelChoice, Provider, SessionOpened, SessionStart, Turn};
use agent_client_protocol::BoxFuture;
use agent_client_protocol::schema::v1::{SessionId, StopReason};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

/// Drives Codex through its official local app-server protocol. Authentication
/// is owned by Codex; discovery requires a ChatGPT login and never reads tokens.
pub struct CodexProvider {
    process: CodexProcess,
    handshake: Handshake,
    effort: Option<String>,
    threads: Mutex<HashMap<String, Arc<CodexThread>>>,
}

impl CodexProvider {
    /// Provider selector, distinct from the OpenAI chat API provider.
    pub const NAME: &'static str = "codex";

    /// Spawn the runtime and discover its login, models, and reasoning efforts.
    pub async fn discover(program: impl Into<PathBuf>) -> Result<Self, AgentError> {
        Self::discover_with(program, Vec::new()).await
    }

    /// Prefix arguments before `app-server`, useful for an executable wrapper
    /// or the fixture runner. All sessions share this retained process.
    pub async fn discover_with(
        program: impl Into<PathBuf>,
        prefix: Vec<String>,
    ) -> Result<Self, AgentError> {
        let process = CodexProcess::spawn(&program.into(), &prefix)?;
        let handshake = Handshake::discover(&process).await?;
        Ok(Self {
            process,
            handshake,
            effort: None,
            threads: Mutex::default(),
        })
    }

    /// Override reasoning effort. It is validated against each turn's model
    /// before that turn sends any inference request to the runtime.
    #[must_use]
    pub fn effort(mut self, effort: impl Into<String>) -> Self {
        self.effort = Some(effort.into());
        self
    }

    /// Add input to the exact turn the caller expects. The runtime atomically
    /// rejects stale `expectedTurnId` values, including during a concurrent turn.
    pub async fn steer(
        &self,
        thread_id: &str,
        expected_turn_id: &str,
        text: &str,
    ) -> Result<(), AgentError> {
        self.thread(thread_id)?;
        if expected_turn_id.is_empty() {
            return Err(AgentError::provider(
                "Codex steering requires expectedTurnId",
            ));
        }
        let response = self.process.request("turn/steer", json!({"threadId":thread_id,"expectedTurnId":expected_turn_id,"input":[{"type":"text","text":text}]})).await?;
        if response["turnId"].as_str() != Some(expected_turn_id) {
            return Err(AgentError::provider(
                "Codex steer response named a different or missing turn id",
            ));
        }
        Ok(())
    }

    /// Read a persisted thread without resuming or sending model input.
    pub async fn read_thread(&self, thread_id: &str) -> Result<ThreadRecord, AgentError> {
        let response = self
            .process
            .request(
                "thread/read",
                json!({"threadId":thread_id,"includeTurns":true}),
            )
            .await?;
        let record = ThreadRecord::parse(&response)?;
        if record.id != thread_id {
            return Err(AgentError::provider(
                "Codex thread/read returned a different thread id",
            ));
        }
        Ok(record)
    }

    fn thread(&self, id: &str) -> Result<Arc<CodexThread>, AgentError> {
        self.threads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()
            .ok_or_else(|| AgentError::UnknownSession(id.into()))
    }

    async fn open(&self, start: SessionStart) -> Result<SessionOpened, AgentError> {
        start.validate(self)?;
        if start.resume
            && self
                .threads
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains_key(&start.session_id.to_string())
        {
            return Err(AgentError::SessionExists(start.session_id.to_string()));
        }
        let default = &self
            .handshake
            .models
            .first()
            .ok_or_else(|| AgentError::provider("Codex has no model catalog"))?
            .id;
        if let Some(model) = start
            .model
            .as_deref()
            .or((!start.resume).then_some(default.as_str()))
        {
            self.handshake
                .effort(model, start.effort.as_deref().or(self.effort.as_deref()))?;
        }
        let params = ThreadSpec::params(&start, default)?;
        let response = self
            .process
            .request(
                if start.resume {
                    "thread/resume"
                } else {
                    "thread/start"
                },
                params,
            )
            .await?;
        let record = ThreadRecord::parse(&response)?;
        if start.resume && record.id != start.session_id.to_string() {
            return Err(AgentError::provider(
                "Codex thread/resume returned a different thread id",
            ));
        }
        if response["modelProvider"]
            .as_str()
            .is_some_and(|provider| provider != "openai")
        {
            return Err(AgentError::provider(
                "Codex refused the OpenAI subscription model provider",
            ));
        }
        let model = record
            .model
            .clone()
            .ok_or_else(|| AgentError::provider("Codex thread response omitted its model"))?;
        if start.model.as_ref().is_some_and(|chosen| chosen != &model) {
            return Err(AgentError::provider(
                "Codex did not apply the requested session model",
            ));
        }
        let effort = start
            .effort
            .as_deref()
            .or(self.effort.as_deref())
            .or(response["reasoningEffort"]
                .as_str()
                .or_else(|| record.raw["reasoningEffort"].as_str()))
            .map(str::to_owned);
        self.handshake.effort(&model, effort.as_deref())?;
        let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
        if threads.contains_key(&record.id) {
            return Err(AgentError::SessionExists(record.id));
        }
        let frames = self.process.subscribe(&record.id)?;
        threads.insert(
            record.id.clone(),
            Arc::new(CodexThread::new(
                record.id.clone(),
                model.clone(),
                effort,
                frames,
            )),
        );
        Ok(SessionOpened::new(SessionId::new(record.id))
            .replay(if start.resume {
                record.replay
            } else {
                Vec::new()
            })
            .model(model))
    }
}

impl Provider for CodexProvider {
    fn capabilities(&self) -> crate::ProviderCapabilities {
        crate::ProviderCapabilities {
            durable_load: true,
            history_replay: true,
            library_steering: true,
            effort_selection: true,
            terminal_cancellation_ack: true,
            tools: true,
            ..crate::ProviderCapabilities::text_session()
        }
    }
    fn requires_terminal_cancellation(&self) -> bool {
        true
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
            let thread = self.thread(&turn.session_id().to_string())?;
            let model = turn.model().unwrap_or(&thread.model);
            let inherited = (model == thread.model)
                .then_some(thread.effort.as_deref())
                .flatten();
            let effort = self
                .handshake
                .effort(model, inherited.or(self.effort.as_deref()))?;
            thread.run(&self.process, &turn, model, &effort).await
        })
    }
}
