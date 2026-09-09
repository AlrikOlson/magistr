//! The judge as a fresh, tool-less ACP session.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    ContentBlock, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigOptionValue, SessionConfigSelectOptions, SessionUpdate,
};
use magistr_acp::{
    AcpEvent, AcpPermissionOptionKind, AcpPermissionRequest, AcpPermissionResponse, AcpTransport,
};
use tokio::sync::mpsc::UnboundedReceiver;

use super::judge::Judge;
use super::judge_input::JudgeInput;
use super::verdict::Verdict;
use crate::agent::{AgentConfig, Backend};
use crate::orchestration::loop_driver::Connector;

/// A [`Judge`] that opens a **new** ACP session per verdict — never the
/// session that did the work — with nothing pre-approved and every
/// permission request refused, selects the smallest model the agent
/// offers, sends exactly [`JudgeInput::prompt`], collects the turn's
/// text and reads the [`Verdict`] off it.
///
/// It cannot fail: a connection that errors, a turn that ends without
/// text, or a judge that has not answered by [`SessionJudge::TIMEOUT`]
/// all answer [`Verdict::not_yet`] with the reason, so the loop
/// iterates rather than wedges.
pub struct SessionJudge {
    connector: Connector,
    cwd: PathBuf,
    backend: Backend,
    model: Option<String>,
    timeout: Duration,
}

impl SessionJudge {
    /// How long a verdict may take, connection included.
    pub const TIMEOUT: Duration = Duration::from_secs(30);

    /// Ids and names that mark an offered model as a small one, in
    /// preference order.
    const SMALL_HINTS: [&'static str; 5] = ["haiku", "fast", "flash", "mini", "small"];

    /// A judge connecting through `connector` for the project at `cwd`.
    #[must_use]
    pub fn new(connector: Connector, cwd: impl Into<PathBuf>, backend: Backend) -> Self {
        Self {
            connector,
            cwd: cwd.into(),
            backend,
            model: None,
            timeout: Self::TIMEOUT,
        }
    }

    /// Select this model id when the agent offers it, instead of the
    /// smallest one it advertises.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Wait at most this long for a verdict.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The `(option id, value)` to set among `options`: the configured
    /// model when offered, else the first offered whose id or name
    /// carries a small-model hint, else `None` for the agent's default.
    fn pick_model(&self, options: &[SessionConfigOption]) -> Option<(String, String)> {
        let option = options
            .iter()
            .find(|o| o.category == Some(SessionConfigOptionCategory::Model))?;
        let SessionConfigKind::Select(select) = &option.kind else {
            return None;
        };
        let offered: Vec<(String, String)> = match &select.options {
            SessionConfigSelectOptions::Ungrouped(choices) => choices.iter().collect::<Vec<_>>(),
            SessionConfigSelectOptions::Grouped(groups) => groups
                .iter()
                .flat_map(|group| group.options.iter())
                .collect(),
            _ => Vec::new(),
        }
        .into_iter()
        .map(|choice| (choice.value.to_string(), choice.name.to_lowercase()))
        .collect();
        let choice = if let Some(wanted) = &self.model {
            offered.iter().find(|(id, _)| id == wanted)
        } else {
            Self::SMALL_HINTS.iter().find_map(|hint| {
                offered
                    .iter()
                    .find(|(id, name)| id.to_lowercase().contains(hint) || name.contains(hint))
            })
        }?;
        Some((option.id.to_string(), choice.0.clone()))
    }

    /// Connect, refuse tools, pick the model, ask, and read the answer;
    /// the session is closed whatever happened.
    async fn ask(&self, prompt: &str) -> Result<Verdict, String> {
        let config = AgentConfig::new(prompt, &self.cwd)
            .backend(self.backend)
            .allowed_tools(Vec::<String>::new());
        let mut transport = self
            .connector
            .connect(config)
            .await
            .map_err(|e| format!("the judge's agent could not be started: {e}"))?;
        let mut events = transport
            .read_events()
            .ok_or_else(|| "the judge's transport has no event channel".to_owned())?;
        let mut permissions = transport.take_permission_rx();
        let verdict = self
            .turn(&transport, &mut events, &mut permissions, prompt)
            .await;
        let _ = transport.close().await;
        verdict
    }

    /// One turn: the handshake's `Initialized` is already queued and the
    /// model must be set before the prompt goes out; then the text
    /// chunks are gathered until the `PromptResponse`.
    async fn turn(
        &self,
        transport: &AcpTransport,
        events: &mut UnboundedReceiver<magistr_acp::Result<AcpEvent>>,
        permissions: &mut Option<UnboundedReceiver<AcpPermissionRequest>>,
        prompt: &str,
    ) -> Result<Verdict, String> {
        let mut text = String::new();
        let mut prompted = false;
        loop {
            tokio::select! {
                event = events.recv() => {
                    let Some(event) = event else {
                        return Err(format!("the judge's session ended before it answered: {text}"));
                    };
                    match event.map_err(|e| format!("the judge's transport failed: {e}"))? {
                        AcpEvent::Initialized { session_id, config_options, .. } if !prompted => {
                            if let Some((option, value)) =
                                self.pick_model(config_options.as_deref().unwrap_or_default())
                            {
                                let value = SessionConfigOptionValue::ValueId { value: value.into() };
                                let _ = transport.set_config_option(session_id, option, value).await;
                            }
                            transport
                                .prompt(prompt)
                                .map_err(|e| format!("the judge could not be asked: {e}"))?;
                            prompted = true;
                        }
                        AcpEvent::SessionUpdate { update, .. } => match *update {
                            SessionUpdate::AgentMessageChunk(chunk) => {
                                if let ContentBlock::Text(block) = chunk.content {
                                    text.push_str(&block.text);
                                }
                            }
                            // The judge's session is not the cockpit's, so
                            // its context is visible nowhere else: on the
                            // log for a measured run.
                            SessionUpdate::UsageUpdate(usage) => tracing::info!(
                                target: "magistr::loop",
                                used = usage.used,
                                size = usage.size,
                                "judge usage"
                            ),
                            _ => {}
                        },
                        AcpEvent::PromptResponse { .. } => return Ok(Verdict::from_text(&text)),
                        _ => {}
                    }
                }
                request = recv(permissions) => {
                    match request {
                        Some(request) => {
                            let answer = refuse(&request);
                            let _ = request.response_tx.send(answer);
                        }
                        None => *permissions = None,
                    }
                }
            }
        }
    }
}

/// The reject option when the agent offers one, else a cancel: the judge
/// has no tools, whatever it asks for.
fn refuse(request: &AcpPermissionRequest) -> AcpPermissionResponse {
    request
        .options
        .iter()
        .find(|option| option.kind == AcpPermissionOptionKind::RejectOnce)
        .map_or(AcpPermissionResponse::Cancelled, |option| {
            AcpPermissionResponse::Selected(option.option_id.clone())
        })
}

/// The next permission request; a missing receiver never resolves, so
/// the `select!` never spins on it.
async fn recv(
    rx: &mut Option<UnboundedReceiver<AcpPermissionRequest>>,
) -> Option<AcpPermissionRequest> {
    match rx {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

impl Judge for SessionJudge {
    fn judge<'a>(
        &'a self,
        input: &'a JudgeInput,
    ) -> Pin<Box<dyn Future<Output = Verdict> + Send + 'a>> {
        Box::pin(async move {
            let prompt = input.prompt();
            let asked = std::time::Instant::now();
            let verdict = match tokio::time::timeout(self.timeout, self.ask(&prompt)).await {
                Ok(Ok(verdict)) => verdict,
                Ok(Err(reason)) => Verdict::not_yet(reason),
                Err(_) => Verdict::not_yet(format!(
                    "the judge did not answer within {} s",
                    self.timeout.as_secs_f64()
                )),
            };
            tracing::info!(
                target: "magistr::loop",
                elapsed = ?asked.elapsed(),
                ok = verdict.ok,
                impossible = verdict.impossible,
                chunk = %input.chunk,
                "judge answered"
            );
            verdict
        })
    }
}

impl std::fmt::Debug for SessionJudge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionJudge")
            .field("cwd", &self.cwd)
            .field("model", &self.model)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "session_judge.tests.rs"]
mod tests;
