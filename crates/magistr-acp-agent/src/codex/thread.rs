//! One serialized turn queue, independent of the provider's other threads.

use super::approval::Approval;
use super::frame::Frame;
use super::process::CodexProcess;
use super::rpc_error::RpcError;
use super::translate::Translator;
use crate::{AgentError, Turn};
use agent_client_protocol::schema::v1::StopReason;
use serde_json::json;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, mpsc};

const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
enum Flight {
    Idle,
    Starting,
    Running(String),
    Broken(String),
}

pub(crate) struct CodexThread {
    pub id: String,
    pub model: String,
    pub effort: Option<String>,
    frames: AsyncMutex<mpsc::UnboundedReceiver<Result<Frame, String>>>,
    flight: Mutex<Flight>,
}

impl CodexThread {
    pub fn new(
        id: String,
        model: String,
        effort: Option<String>,
        frames: mpsc::UnboundedReceiver<Result<Frame, String>>,
    ) -> Self {
        Self {
            id,
            model,
            effort,
            frames: AsyncMutex::new(frames),
            flight: Mutex::new(Flight::Idle),
        }
    }

    pub async fn run(
        &self,
        process: &CodexProcess,
        turn: &Turn,
        model: &str,
        effort: &str,
    ) -> Result<StopReason, AgentError> {
        let mut frames = self.frames.lock().await;
        self.recover(process, &mut frames).await?;
        if turn.is_cancelled() {
            return Ok(StopReason::Cancelled);
        }
        self.set_flight(Flight::Starting);
        let response = process.request("turn/start", json!({"threadId":self.id,"input":[{"type":"text","text":turn.text()}],"model":model,"effort":effort})).await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                // The runtime may have accepted a timed-out request. Keep
                // Starting so the next caller must drain or fail closed.
                if !process.has_active_turn(&self.id) {
                    self.set_flight(Flight::Idle);
                }
                return Err(error);
            }
        };
        let id = response["turn"]["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                AgentError::provider("malformed Codex turn/start response: missing turn id")
            })?
            .to_owned();
        self.set_flight(Flight::Running(id.clone()));
        let mut translator = Translator::default();
        let mut interrupted = false;
        let mut deadline = None;
        let mut reported_error = None;
        loop {
            let frame = tokio::select! {
                frame = receive(&mut frames) => frame?,
                () = turn.cancelled(), if !interrupted => {
                    interrupted = true;
                    deadline = Some(tokio::time::Instant::now() + DRAIN_TIMEOUT);
                    process.interrupt(&self.id, &id).await?;
                    continue;
                }
                () = wait_deadline(deadline), if deadline.is_some() => {
                    let reason = "Codex interrupt never reached terminal turn/completed; thread cannot be reused";
                    self.set_flight(Flight::Broken(reason.into()));
                    return Err(AgentError::provider(reason));
                }
            };
            if frame.turn_id().is_some_and(|frame_id| frame_id != id) {
                cancel_request(process, &frame).await?;
                continue;
            }
            match frame {
                Frame::Notification { method, params } if method == "turn/completed" => {
                    if params["turn"]["id"].as_str() != Some(id.as_str()) {
                        return Err(AgentError::provider(
                            "malformed Codex terminal notification: missing turn id",
                        ));
                    }
                    let status = params["turn"]["status"].as_str().ok_or_else(|| {
                        AgentError::provider(
                            "malformed Codex terminal notification: missing status",
                        )
                    })?;
                    let result = match status {
                        "completed" => Ok(if interrupted {
                            StopReason::Cancelled
                        } else {
                            StopReason::EndTurn
                        }),
                        "interrupted" => Ok(StopReason::Cancelled),
                        "failed" => {
                            Err(AgentError::provider(if params["turn"]["error"].is_null() {
                                reported_error.unwrap_or_else(|| {
                                    "Codex turn failed without error details".into()
                                })
                            } else {
                                RpcError::turn_failure(&params["turn"]["error"])
                            }))
                        }
                        status => {
                            return Err(AgentError::provider(format!(
                                "unknown Codex terminal turn status: {status}"
                            )));
                        }
                    };
                    self.set_flight(Flight::Idle);
                    return result;
                }
                Frame::Notification { method, params } if method == "error" => {
                    reported_error = Some(RpcError::turn_failure(&params["error"]));
                }
                Frame::Notification { method, params } => {
                    for update in translator.notification(&method, &params)? {
                        turn.update(update)?;
                    }
                }
                Frame::Request {
                    id: request_id,
                    method,
                    params,
                } => {
                    let item = params["itemId"]
                        .as_str()
                        .and_then(|id| translator.items.get(id));
                    let parsed = if method == "mcpServer/elicitation/request" {
                        Approval::elicitation(&request_id, &params)
                    } else {
                        Approval::parse(&method, &params, item)
                    };
                    let ask = match parsed {
                        Ok(ask) => ask,
                        Err(error) => {
                            process.write(&json!({"id":request_id,"error":{"code":-32602,"message":error.to_string()}})).await?;
                            return Err(error);
                        }
                    };
                    let outcome = tokio::select! {
                        outcome = turn.request_permission(ask.tool_call(), ask.options()) => outcome,
                        failure = process.failed() => return Err(AgentError::provider(failure)),
                    };
                    let answer = match &outcome {
                        Ok(outcome) => ask.answer(outcome),
                        Err(_) => Approval::cancel(&method),
                    };
                    process
                        .write(&json!({"id":request_id,"result":answer}))
                        .await?;
                    outcome?;
                }
                _ => {}
            }
        }
    }

    fn set_flight(&self, flight: Flight) {
        *self.flight.lock().unwrap_or_else(PoisonError::into_inner) = flight;
    }

    async fn recover(
        &self,
        process: &CodexProcess,
        frames: &mut mpsc::UnboundedReceiver<Result<Frame, String>>,
    ) -> Result<(), AgentError> {
        let flight = self
            .flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let mut active = match flight {
            Flight::Idle => return Ok(()),
            Flight::Broken(reason) => return Err(AgentError::provider(reason)),
            Flight::Starting => None,
            Flight::Running(id) => Some(id),
        };
        let recovered = tokio::time::timeout(DRAIN_TIMEOUT, async {
            if let Some(id) = &active
                && process.has_active_turn(&self.id)
            {
                process.interrupt(&self.id, id).await?;
            }
            loop {
                let frame = receive(frames).await?;
                cancel_request(process, &frame).await?;
                if let Frame::Notification { method, params } = &frame {
                    if active.is_none() && method == "turn/started" {
                        let id = params["turn"]["id"]
                            .as_str()
                            .ok_or_else(|| {
                                AgentError::provider("malformed Codex turn/started notification")
                            })?
                            .to_owned();
                        process.interrupt(&self.id, &id).await?;
                        active = Some(id);
                    }
                    if method == "turn/completed"
                        && frame.turn_id() == active.as_deref()
                        && active.is_some()
                    {
                        return Ok::<_, AgentError>(());
                    }
                }
            }
        })
        .await;
        match recovered {
            Ok(Ok(())) => {
                self.set_flight(Flight::Idle);
                Ok(())
            }
            result => {
                let reason = match result {
                    Ok(Err(error)) => format!("Codex stale turn recovery failed: {error}"),
                    _ => "Codex stale turn did not terminate; thread cannot be reused".into(),
                };
                self.set_flight(Flight::Broken(reason.clone()));
                Err(AgentError::provider(reason))
            }
        }
    }
}

async fn receive(
    frames: &mut mpsc::UnboundedReceiver<Result<Frame, String>>,
) -> Result<Frame, AgentError> {
    frames
        .recv()
        .await
        .ok_or_else(|| AgentError::provider("Codex app-server disconnected mid-turn"))?
        .map_err(AgentError::provider)
}

async fn cancel_request(process: &CodexProcess, frame: &Frame) -> Result<(), AgentError> {
    if let Frame::Request { id, method, .. } = frame {
        process
            .write(&json!({"id":id,"result":Approval::cancel(method)}))
            .await?;
    }
    Ok(())
}

async fn wait_deadline(deadline: Option<tokio::time::Instant>) {
    if let Some(deadline) = deadline {
        tokio::time::sleep_until(deadline).await;
    }
}
