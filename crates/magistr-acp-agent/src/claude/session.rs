//! One ACP session on one `claude` process.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex as SyncMutex, PoisonError};
use std::time::Duration;

use agent_client_protocol::schema::v1::StopReason;
use tokio::sync::Mutex;

use crate::claude::control::ControlRequest;
use crate::claude::frame::Frame;
use crate::claude::launch::LaunchSpec;
use crate::claude::permission::PermissionAsk;
use crate::claude::process::ClaudeCli;
use crate::claude::translate::{Step, TurnEnd, TurnTranslator};
use crate::{AgentError, Turn};

/// How long a stale turn — one whose provider future was dropped before its
/// result arrived — may take to finish before the next turn starts anyway.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// A session: the process, the translator that carries its state across
/// turns, the model it is on, and whether a turn is still in flight on the
/// wire.
///
/// Turns run one at a time (the agent's worker guarantees it). A turn writes
/// the user message and reads frames until the `result`, relaying each
/// permission ask to the client as it comes; a cancel sends `interrupt` and
/// keeps reading to the result, which then ends the turn cancelled. If the
/// worker drops the turn's future before that result (its cancel grace ran
/// out), the session remembers the turn is in flight and drains it before
/// the next one — a stale `result` must never end a fresh turn.
pub(crate) struct ClaudeSession {
    cli: ClaudeCli,
    translator: Mutex<TurnTranslator>,
    model: SyncMutex<Option<String>>,
    in_flight: AtomicBool,
}

impl ClaudeSession {
    /// Spawn the process for `spec` and run the initialize handshake.
    pub async fn open(
        program: &Path,
        prefix: &[String],
        spec: &LaunchSpec,
    ) -> Result<Self, AgentError> {
        let mut args: Vec<String> = prefix.to_vec();
        args.extend(spec.args());
        let cli = ClaudeCli::spawn(program, &args, &spec.cwd)?;
        let handshake = cli.control(ControlRequest::Initialize).await?;
        if let Some(error) = handshake.error {
            return Err(AgentError::provider(format!(
                "claude refused initialize: {error}"
            )));
        }
        Ok(Self {
            cli,
            translator: Mutex::new(TurnTranslator::new()),
            model: SyncMutex::new(spec.model.clone()),
            in_flight: AtomicBool::new(false),
        })
    }

    /// Run one turn to its end.
    pub async fn run_turn(&self, turn: &Turn) -> Result<StopReason, AgentError> {
        let mut translator = self.translator.lock().await;
        if self.in_flight.load(Ordering::SeqCst) {
            self.drain_stale(&mut translator).await;
        }
        self.switch_model(turn.model()).await?;
        translator.next_turn();
        self.in_flight.store(true, Ordering::SeqCst);
        self.cli
            .write(&ControlRequest::user_message(&turn.text()))
            .await?;

        loop {
            let frame = tokio::select! {
                frame = self.cli.next_frame() => frame,
                () = turn.cancelled(), if !translator.is_interrupted() => {
                    translator.mark_interrupted();
                    self.cli.control_and_forget(ControlRequest::Interrupt).await?;
                    continue;
                }
            };
            let Some(frame) = frame else {
                self.in_flight.store(false, Ordering::SeqCst);
                return Err(AgentError::provider(format!(
                    "claude exited mid-turn{}",
                    self.cli.stderr_note()
                )));
            };
            let steps = translator.translate(&frame);
            if matches!(frame, Frame::Init(_)) {
                tracing::info!(
                    session = translator.session_id().unwrap_or_default(),
                    model = translator.model().unwrap_or_default(),
                    mcp_servers = ?translator.mcp_servers(),
                    "claude turn open"
                );
            }
            for step in steps {
                match step {
                    Step::Update(update) => turn.update(*update)?,
                    Step::Ask { request_id, ask } => {
                        self.relay_ask(turn, &request_id, &ask).await?
                    }
                    Step::Refuse {
                        request_id,
                        subtype,
                    } => {
                        self.cli
                            .write(&ControlRequest::response(
                                &request_id,
                                Err(format!("magistr does not serve {subtype}")),
                            ))
                            .await?;
                    }
                    Step::Done(end) => {
                        self.in_flight.store(false, Ordering::SeqCst);
                        return match end {
                            TurnEnd::Stop(stop) => Ok(stop),
                            TurnEnd::Failed(reason) => Err(AgentError::provider(reason)),
                        };
                    }
                }
            }
        }
    }

    /// Ask the client and answer the CLI. A cancel while the ask is open
    /// denies it and asks the CLI to interrupt.
    async fn relay_ask(
        &self,
        turn: &Turn,
        request_id: &str,
        ask: &PermissionAsk,
    ) -> Result<(), AgentError> {
        let outcome = turn
            .request_permission(ask.tool_call(), ask.options())
            .await?;
        self.cli
            .write(&ControlRequest::response(
                request_id,
                Ok(ask.answer(&outcome)),
            ))
            .await
    }

    /// Read to the stale turn's result (or give up after a while).
    async fn drain_stale(&self, translator: &mut TurnTranslator) {
        let drained = tokio::time::timeout(DRAIN_TIMEOUT, async {
            while let Some(frame) = self.cli.next_frame().await {
                if let Frame::ControlRequest(request) = &frame {
                    let _ = self
                        .cli
                        .write(&ControlRequest::response(
                            &request.request_id,
                            Ok(serde_json::json!({
                                "behavior": "deny",
                                "message": "The turn was cancelled",
                                "interrupt": true,
                            })),
                        ))
                        .await;
                    continue;
                }
                let ended = translator
                    .translate(&frame)
                    .into_iter()
                    .any(|step| matches!(step, Step::Done(_)));
                if ended {
                    return;
                }
            }
        })
        .await;
        if drained.is_err() {
            tracing::warn!("a cancelled turn never reported its result; moving on");
        }
        self.in_flight.store(false, Ordering::SeqCst);
    }

    /// Tell the CLI about a model the client chose since the last turn.
    async fn switch_model(&self, wanted: Option<&str>) -> Result<(), AgentError> {
        let current = self
            .model
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if wanted == current.as_deref() {
            return Ok(());
        }
        let model = wanted
            .filter(|m| *m != LaunchSpec::DEFAULT_MODEL)
            .map(str::to_owned);
        let response = self.cli.control(ControlRequest::SetModel(model)).await?;
        if let Some(error) = response.error {
            return Err(AgentError::provider(format!(
                "claude refused the model: {error}"
            )));
        }
        *self.model.lock().unwrap_or_else(PoisonError::into_inner) = wanted.map(str::to_owned);
        Ok(())
    }
}
