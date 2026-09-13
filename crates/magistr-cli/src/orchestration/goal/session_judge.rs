//! The judge as a fresh, tool-less session on the shared session owner.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use magistr_core::ContentBlock;

use super::judge::Judge;
use super::judge_evidence::JudgeEvidence;
use super::judge_input::JudgeInput;
use super::no_tools::NoTools;
use super::verdict::Verdict;
use crate::agent::message::AgentMessage;
use crate::agent::session_supervisor::SessionSupervisor;
use crate::agent::{AgentConfig, Backend};
use crate::orchestration::loop_driver::Connector;
use crate::permissions::NoninteractivePolicy;

/// A [`Judge`] that opens a **new** ACP session per verdict — never the
/// session that did the work — on the shared session owner
/// ([`SessionSupervisor`]), with nothing pre-approved and a policy that
/// refuses every permission request ([`NoTools`]), uses the resolved
/// session model, sends exactly [`JudgeInput::prompt`], collects the
/// turn's text and reads the [`Verdict`] off it only once the owner's
/// evidence admits it ([`JudgeEvidence`]): a confirmed `EndTurn` over a
/// session whose cleanup settled.
///
/// It cannot fail: a connection that errors, a turn that ends without
/// a terminal response — cancelled, limited, refused, its stream lost —
/// a cleanup that does not settle, or a judge that has not answered by
/// [`SessionJudge::TIMEOUT`] all answer [`Verdict::not_yet`] with the
/// reason, so the loop iterates rather than wedges.
pub struct SessionJudge {
    connector: Connector,
    cwd: PathBuf,
    backend: Backend,
    model: Option<String>,
    timeout: Duration,
}

impl SessionJudge {
    /// How long a verdict may take, connection and cleanup included.
    pub const TIMEOUT: Duration = Duration::from_secs(30);

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

    /// Request this model id through the shared startup policy.
    /// An unavailable or refused model prevents inference.
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

    /// Connect with resolved setup, hand the transport to the shared
    /// owner under the refusing policy, gather the answer, then take the
    /// owner's evidence once the session is gone.
    async fn ask(&self, prompt: &str) -> Result<Verdict, String> {
        let mut config = AgentConfig::new(prompt, &self.cwd)
            .backend(self.backend)
            .allowed_tools(Vec::<String>::new());
        config.model.clone_from(&self.model);
        // The connector resolves the provider, the model and its
        // governance for this session; the permission policy it resolved
        // is not the judge's — the judge refuses everything.
        let (transport, _spec) = self
            .connector
            .connect(config)
            .await
            .map_err(|e| format!("the judge's agent could not be started: {e}"))?;
        let (mut owner, mut messages) = SessionSupervisor::start_connected(
            prompt,
            transport,
            Some((Box::new(NoTools), NoninteractivePolicy::Refuse)),
        )
        .await
        .map_err(|e| format!("the judge's session could not be opened: {e}"))?;
        let text = answer(&mut messages).await;
        // The declared cleanup settlement, then the outcome the owner
        // confirmed: only together do they admit the text.
        let cleanup = owner
            .terminate_session()
            .await
            .map_err(|error| error.to_string());
        let outcome = owner.snapshot().last_outcome;
        JudgeEvidence { outcome, cleanup }.admit(&text)
    }
}

/// The turn's text as the owner relays it, gathered until the owner
/// reports the turn's end — a terminal message, a failure, or the end of
/// observation. What ended it is the owner's to say, in its snapshot.
async fn answer(
    messages: &mut crate::agent::observation::ObservationReceiver<
        crate::error::Result<AgentMessage>,
    >,
) -> String {
    let mut text = String::new();
    while let Some(message) = messages.recv().await {
        match message {
            Ok(AgentMessage::Assistant { message, .. }) => {
                for block in message.content {
                    if let ContentBlock::Text { text: chunk } = block {
                        text.push_str(&chunk);
                    }
                }
            }
            // The judge's session is not the cockpit's, so its context
            // is visible nowhere else: on the log for a measured run.
            Ok(AgentMessage::UsageUpdate {
                context_tokens_used,
                context_window_size,
                ..
            }) => tracing::info!(
                target: "magistr::loop",
                used = context_tokens_used,
                size = context_window_size,
                "judge usage"
            ),
            Ok(AgentMessage::Result { .. } | AgentMessage::Cancelled { .. }) | Err(_) => break,
            Ok(_) => {}
        }
    }
    text
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
