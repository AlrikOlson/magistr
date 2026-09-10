//! The one provider every chat API is served through.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use agent_client_protocol::BoxFuture;
use agent_client_protocol::schema::v1::{Cost, SessionId, StopReason, UsageUpdate};

use crate::api::api_session::ApiSession;
use crate::api::line_buffer::LineBuffer;
use crate::{
    AgentError, ChatApi, ModelChoice, ModelSpec, Provider, SessionOpened, SessionStart,
    StreamEvent, Turn,
};

/// [`Provider`] over any [`ChatApi`]: keeps each session's history in
/// memory, sends the whole of it on every turn, streams the reply's text as
/// `agent_message_chunk`s as the body arrives, closes the turn with a
/// `usage_update` (the turn's tokens against the model's window, the
/// session's cumulative cost at the model's pricing) and answers a
/// `session/cancel` by dropping the HTTP response — which aborts the
/// stream — and reporting `cancelled`.
///
/// No tool harness: a turn is one request and one reply, and nothing here
/// ever asks the client for a permission.
pub struct ApiProvider<A> {
    api: A,
    http: reqwest::Client,
    sessions: Mutex<HashMap<SessionId, Arc<Mutex<ApiSession>>>>,
}

impl<A: ChatApi> ApiProvider<A> {
    /// Serve `api`.
    pub fn new(api: A) -> Self {
        Self {
            api,
            http: reqwest::Client::new(),
            sessions: Mutex::default(),
        }
    }

    /// The API underneath.
    #[must_use]
    pub fn api(&self) -> &A {
        &self.api
    }

    fn session(&self, id: &SessionId) -> Result<Arc<Mutex<ApiSession>>, AgentError> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()
            .ok_or_else(|| AgentError::UnknownSession(id.to_string()))
    }

    fn spec(&self, model: &str) -> Result<&ModelSpec, AgentError> {
        self.api
            .models()
            .iter()
            .find(|m| m.id == model)
            .ok_or_else(|| AgentError::UnknownModel(model.to_owned()))
    }

    async fn run_turn(&self, turn: Turn) -> Result<StopReason, AgentError> {
        let name = self.api.name();
        let session = self.session(turn.session_id())?;
        let model = turn
            .model()
            .ok_or_else(|| AgentError::provider(format!("{name}: no model selected")))?
            .to_owned();
        let spec = self.spec(&model)?;
        let history = lock(&session).begin_turn(turn.text());
        let request = self.api.request(&self.http, &model, &history);

        let sent = tokio::select! {
            sent = request.send() => sent,
            () = turn.cancelled() => {
                lock(&session).abandon_turn(String::new());
                return Ok(StopReason::Cancelled);
            }
        };
        let response = match sent {
            Ok(response) => response,
            Err(e) => {
                lock(&session).abandon_turn(String::new());
                return Err(AgentError::provider(format!("{name}: request failed: {e}")));
            }
        };
        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".to_owned());
            lock(&session).abandon_turn(String::new());
            return Err(AgentError::provider(format!(
                "{name} API error {status}: {body}"
            )));
        }

        let mut response = response;
        let mut lines = LineBuffer::default();
        let mut reply = String::new();
        let mut usage = None;
        loop {
            let next = tokio::select! {
                chunk = response.chunk() => Some(chunk),
                () = turn.cancelled() => None,
            };
            let Some(chunk) = next else {
                // Dropping the response closes the connection: the stream
                // is aborted, not drained.
                drop(response);
                lock(&session).abandon_turn(reply);
                return Ok(StopReason::Cancelled);
            };
            let chunk =
                chunk.map_err(|e| AgentError::provider(format!("{name}: stream failed: {e}")))?;
            let Some(bytes) = chunk else {
                if let Some(tail) = lines.finish()? {
                    self.absorb(&turn, &tail, &mut reply, &mut usage)?;
                }
                break;
            };
            lines.push(&bytes);
            for line in lines.drain_lines()? {
                self.absorb(&turn, &line, &mut reply, &mut usage)?;
            }
        }

        let (input, output) = usage.unwrap_or((0, 0));
        let cost = spec.pricing.cost(input, output);
        let total = {
            let mut session = lock(&session);
            session.end_turn(reply, usage, cost);
            session.cost_usd
        };
        turn.usage(
            UsageUpdate::new(input + output, spec.context_window).cost(Cost::new(total, "USD")),
        )?;
        Ok(StopReason::EndTurn)
    }

    /// Decode one line and act on it: text goes to the client at once and
    /// into the reply; usage is remembered for the end of the turn.
    fn absorb(
        &self,
        turn: &Turn,
        line: &str,
        reply: &mut String,
        usage: &mut Option<(u64, u64)>,
    ) -> Result<(), AgentError> {
        for event in self.api.decode(line)? {
            match event {
                StreamEvent::Text(text) => {
                    reply.push_str(&text);
                    turn.message(text)?;
                }
                StreamEvent::Usage { input, output } => *usage = Some((input, output)),
            }
        }
        Ok(())
    }
}

fn lock(session: &Mutex<ApiSession>) -> std::sync::MutexGuard<'_, ApiSession> {
    session.lock().unwrap_or_else(PoisonError::into_inner)
}

impl<A: ChatApi> Provider for ApiProvider<A> {
    fn capabilities(&self) -> crate::ProviderCapabilities {
        crate::ProviderCapabilities::text_session()
    }
    fn name(&self) -> &str {
        self.api.name()
    }

    fn models(&self) -> Vec<ModelChoice> {
        self.api.models().iter().map(ModelSpec::choice).collect()
    }

    fn start(&self, session: SessionStart) -> BoxFuture<'_, Result<SessionOpened, AgentError>> {
        Box::pin(async move {
            session.validate(self)?;
            self.sessions
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(session.session_id.clone(), Arc::default());
            let mut opened = SessionOpened::new(session.session_id);
            opened.model = session.model;
            Ok(opened)
        })
    }

    fn prompt(&self, turn: Turn) -> BoxFuture<'_, Result<StopReason, AgentError>> {
        Box::pin(self.run_turn(turn))
    }
}
