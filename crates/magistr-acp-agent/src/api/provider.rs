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
            .ok_or_else(|| AgentError::provider(format!("{name}: no model selected")))?;
        let spec = self.spec(model)?;
        let history = lock(&session).begin_turn(turn.text());
        let request = self.api.request(&self.http, model, &history);
        let mut state = ReplyState::default();
        let result = tokio::select! {
            biased;
            () = turn.cancelled() => Ok(StopReason::Cancelled),
            result = self.receive(&turn, request, &mut state) => result,
        };
        let stop = match result {
            Ok(stop) if stop != StopReason::Cancelled => stop,
            other => {
                // Includes request, HTTP, transport, decoding and incomplete-stream
                // failures. Keep only the partial answer the client actually saw.
                lock(&session).abandon_turn(state.reply);
                return other;
            }
        };
        let cost = state
            .usage
            .map_or(0.0, |(input, output)| spec.pricing.cost(input, output));
        let total = {
            let mut session = lock(&session);
            session.end_turn(state.reply, state.usage, cost);
            session.cost_usd
        };
        if let Some((input, output)) = state.usage {
            turn.usage(
                UsageUpdate::new(input.saturating_add(output), spec.context_window)
                    .cost(Cost::new(total, "USD")),
            )?;
        }
        Ok(stop)
    }

    async fn receive(
        &self,
        turn: &Turn,
        request: reqwest::RequestBuilder,
        state: &mut ReplyState,
    ) -> Result<StopReason, AgentError> {
        let name = self.api.name();
        let mut response = request
            .send()
            .await
            .map_err(|e| AgentError::provider(format!("{name}: request failed: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".to_owned());
            return Err(AgentError::provider(format!(
                "{name} API error {status}: {body}"
            )));
        }
        let mut lines = LineBuffer::default();
        while let Some(bytes) = response
            .chunk()
            .await
            .map_err(|e| AgentError::provider(format!("{name}: stream failed: {e}")))?
        {
            lines.push(&bytes);
            for line in lines.drain_lines()? {
                self.absorb(turn, &line, state)?;
            }
        }
        if let Some(tail) = lines.finish()? {
            self.absorb(turn, &tail, state)?;
        }
        if self.api.requires_done() && !state.done {
            return Err(AgentError::provider(format!(
                "{name}: stream ended without end sentinel"
            )));
        }
        let stop = state.finish.ok_or_else(|| {
            AgentError::provider(format!("{name}: stream ended without provider finish"))
        })?;
        Ok(if state.refused && stop == StopReason::EndTurn {
            StopReason::Refusal
        } else {
            stop
        })
    }

    /// Text is forwarded immediately; accounting and terminal evidence remain
    /// separate until the complete response has been validated.
    fn absorb(&self, turn: &Turn, line: &str, state: &mut ReplyState) -> Result<(), AgentError> {
        for event in self.api.decode(line)? {
            if state.done {
                return Err(AgentError::provider(format!(
                    "{}: event after end sentinel",
                    self.api.name()
                )));
            }
            let is_refusal = matches!(event, StreamEvent::Refusal(_));
            match event {
                StreamEvent::Text(_) | StreamEvent::Refusal(_) if state.finish.is_some() => {
                    return Err(AgentError::provider(format!(
                        "{}: text after provider finish",
                        self.api.name()
                    )));
                }
                StreamEvent::Text(text) | StreamEvent::Refusal(text) => {
                    state.refused |= is_refusal;
                    turn.message(text.as_str())?;
                    state.reply.push_str(&text);
                }
                StreamEvent::Usage { input, output } => state.usage = Some((input, output)),
                StreamEvent::Finish(stop) => {
                    if state.finish.replace(stop).is_some() {
                        return Err(AgentError::provider(format!(
                            "{}: duplicate provider finish",
                            self.api.name()
                        )));
                    }
                }
                StreamEvent::Done => {
                    if state.finish.is_none() {
                        return Err(AgentError::provider(format!(
                            "{}: end sentinel without provider finish",
                            self.api.name()
                        )));
                    }
                    state.done = true;
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct ReplyState {
    reply: String,
    usage: Option<(u64, u64)>,
    finish: Option<StopReason>,
    refused: bool,
    done: bool,
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
