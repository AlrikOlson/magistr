use super::*;
use agent_client_protocol::BoxFuture;
use agent_client_protocol::schema::v1::{SessionConfigOptionValue, StopReason};
use magistr_acp_agent::{
    AgentError, ModelChoice, Provider, ScriptedProvider, SessionOpened, SessionStart, Turn,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{Notify, Semaphore, mpsc};

#[path = "session_supervisor/activity.tests.rs"]
mod activity;
#[path = "session_supervisor/goal.tests.rs"]
mod goals;
#[path = "session_supervisor/history.tests.rs"]
mod history;
#[path = "session_supervisor/spending.tests.rs"]
mod spending;

const DEADLINE: Duration = Duration::from_secs(3);

#[tokio::test]
async fn rejected_attachment_keeps_session_available_and_reaches_no_provider() {
    let mut fixture = Fixture::start("first").await;
    fixture.next_started().await;
    fixture.next_terminal().await.expect("first done");
    fixture
        .supervisor
        .send_followup("Read @missing.rs")
        .expect("message admitted");
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Result { is_error: true, .. })
    ));
    assert_eq!(
        fixture.supervisor.snapshot().residency,
        SessionResidency::Resident
    );
    assert_eq!(fixture.supervisor.snapshot().active_turn, None);
    assert!(
        fixture.started.try_recv().is_err(),
        "invalid attachment never reached provider"
    );
    fixture
        .supervisor
        .send_followup("try again")
        .expect("same session admits another turn");
    assert_eq!(fixture.next_started().await.0, "try again");
    fixture.next_terminal().await.expect("second done");
    fixture.close().await;
}

struct Observed {
    started: mpsc::UnboundedSender<(String, Option<String>)>,
    active: AtomicUsize,
    maximum_active: AtomicUsize,
    finish: Semaphore,
    cancellation_seen: Notify,
    acknowledge_cancel: Semaphore,
}

struct ActiveTurn(Arc<Observed>);

impl Drop for ActiveTurn {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

struct ControlledProvider(Arc<Observed>);

impl Provider for ControlledProvider {
    fn capabilities(&self) -> magistr_acp_agent::ProviderCapabilities {
        magistr_acp_agent::ProviderCapabilities {
            terminal_cancellation_ack: true,
            ..magistr_acp_agent::ProviderCapabilities::text_session()
        }
    }

    fn name(&self) -> &str {
        "supervisor-test"
    }

    fn models(&self) -> Vec<ModelChoice> {
        ScriptedProvider.models()
    }

    fn start(
        &self,
        session: SessionStart,
    ) -> BoxFuture<'_, std::result::Result<SessionOpened, AgentError>> {
        ScriptedProvider.start(session)
    }

    fn prompt(&self, turn: Turn) -> BoxFuture<'_, std::result::Result<StopReason, AgentError>> {
        let observed = Arc::clone(&self.0);
        Box::pin(async move {
            let active = observed.active.fetch_add(1, Ordering::SeqCst) + 1;
            observed.maximum_active.fetch_max(active, Ordering::SeqCst);
            let _active = ActiveTurn(Arc::clone(&observed));
            let text = turn.text();
            let _ = observed
                .started
                .send((text.clone(), turn.model().map(str::to_owned)));
            turn.message(format!("started: {text}"))?;
            if text.starts_with("cancel") {
                turn.cancelled().await;
                observed.cancellation_seen.notify_one();
                let permit = observed
                    .acknowledge_cancel
                    .acquire()
                    .await
                    .expect("ACK gate");
                permit.forget();
                turn.message("last update before cancellation acknowledgement")?;
                return Ok(StopReason::Cancelled);
            }
            if text.starts_with("hold") {
                tokio::select! {
                    permit = observed.finish.acquire() => permit.expect("finish gate").forget(),
                    () = turn.cancelled() => return Ok(StopReason::Cancelled),
                }
            }
            {
                use agent_client_protocol::schema::v1::{Cost, UsageUpdate};
                let price = match text.as_str() {
                    "priced" => Some(0.25),
                    "zero" => Some(0.0),
                    "higher" => Some(0.75),
                    "priced fail" => Some(0.5),
                    _ => None,
                };
                if let Some(price) = price {
                    turn.usage(UsageUpdate::new(4, 100).cost(Cost::new(price, "USD")))?;
                } else if text == "unpriced usage" {
                    turn.usage(UsageUpdate::new(8, 100))?;
                }
            }
            if text == "fail" || text == "priced fail" {
                return Err(AgentError::provider("controlled turn failure"));
            }
            if text == "hold max tokens" {
                return Ok(StopReason::MaxTokens);
            }
            if text == "flood text" {
                // Two megabytes in kilobyte deltas, far past the observer's
                // text bound, then the terminal the observer must still get.
                let chunk = "x".repeat(1024);
                for _ in 0..2048 {
                    turn.message(chunk.clone())?;
                }
            }
            if text == "flood tools" {
                for index in 0..64 {
                    turn.tool_call(agent_client_protocol::schema::v1::ToolCall::new(
                        format!("flood-{index}"),
                        "flood",
                    ))?;
                }
            }
            turn.message(format!("finished: {text}"))?;
            Ok(StopReason::EndTurn)
        })
    }
}

struct Fixture {
    supervisor: SessionSupervisor,
    messages: crate::agent::observation::ObservationReceiver<crate::error::Result<AgentMessage>>,
    started: mpsc::UnboundedReceiver<(String, Option<String>)>,
    observed: Arc<Observed>,
    server: tokio::task::JoinHandle<()>,
    _directory: tempfile::TempDir,
}

impl Fixture {
    async fn start(prompt: &str) -> Self {
        Self::start_bounded(
            prompt,
            crate::agent::observation::ObservationCapacity::DEFAULT,
            None,
        )
        .await
    }

    async fn start_bounded(
        prompt: &str,
        capacity: crate::agent::observation::ObservationCapacity,
        max_turns: Option<u32>,
    ) -> Self {
        Self::start_recording(prompt, capacity, max_turns, None).await
    }

    async fn start_recording(
        prompt: &str,
        capacity: crate::agent::observation::ObservationCapacity,
        max_turns: Option<u32>,
        recording: Option<(
            super::super::persistence::SessionHistory,
            crate::config::inputs::ProjectInputs,
        )>,
    ) -> Self {
        let (client, agent) = agent_client_protocol::Channel::duplex();
        let (started, receiver) = mpsc::unbounded_channel();
        let observed = Arc::new(Observed {
            started,
            active: AtomicUsize::new(0),
            maximum_active: AtomicUsize::new(0),
            finish: Semaphore::new(0),
            cancellation_seen: Notify::new(),
            acknowledge_cancel: Semaphore::new(0),
        });
        let provider = ControlledProvider(Arc::clone(&observed));
        let server = tokio::spawn(async move {
            let _ = magistr_acp_agent::serve(provider, agent).await;
        });
        let directory = tempfile::tempdir().expect("session directory");
        let root = recording
            .as_ref()
            .map(|(_, inputs)| inputs.project_root())
            .unwrap_or(directory.path());
        if let Some((history, inputs)) = &recording {
            history
                .begin(
                    inputs,
                    root,
                    "supervisor-test",
                    Some(ScriptedProvider::SLOW),
                )
                .await
                .unwrap();
        }
        let mut transport = magistr_acp::AcpTransport::new_with_args(
            std::path::PathBuf::from("unused-in-process-agent"),
            Vec::new(),
            Some(root.to_path_buf()),
        )
        .expect("transport")
        .session_model(Some(ScriptedProvider::SLOW.into()));
        transport.connect_over(client).await.expect("ACP handshake");
        let (supervisor, messages) = SessionSupervisor::start_connected_bounded_with_limit(
            prompt,
            transport,
            None,
            capacity,
            max_turns,
            recording.map(|(history, _)| history),
        )
        .await
        .expect("supervisor startup");
        Self {
            supervisor,
            messages,
            started: receiver,
            observed,
            server,
            _directory: directory,
        }
    }

    async fn next_started(&mut self) -> (String, Option<String>) {
        tokio::time::timeout(DEADLINE, self.started.recv())
            .await
            .expect("provider starts an admitted turn")
            .expect("provider observation channel")
    }

    async fn next_terminal(&mut self) -> crate::error::Result<AgentMessage> {
        tokio::time::timeout(DEADLINE, async {
            loop {
                let message = self.messages.recv().await.expect("supervisor event stream");
                if matches!(
                    &message,
                    Err(_) | Ok(AgentMessage::Result { .. } | AgentMessage::Cancelled { .. })
                ) {
                    return message;
                }
            }
        })
        .await
        .expect("terminal observation")
    }

    async fn close(&mut self) {
        tokio::time::timeout(DEADLINE, self.supervisor.terminate_session())
            .await
            .expect("supervisor cleanup finishes")
            .expect("supervisor cleanup succeeds");
        tokio::time::timeout(DEADLINE, &mut self.server)
            .await
            .expect("cleanup closes the real ACP connection")
            .expect("controlled provider task ends");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[tokio::test]
async fn selected_model_precedes_initial_prompt_and_queued_turns_are_serial() {
    let mut fixture = Fixture::start("hold first").await;
    assert_eq!(
        fixture.next_started().await,
        ("hold first".into(), Some(ScriptedProvider::SLOW.into()))
    );
    fixture.supervisor.send_followup("hold second").unwrap();
    fixture.supervisor.send_followup("third").unwrap();
    fixture.observed.finish.add_permits(1);
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Result {
            is_error: false,
            ..
        })
    ));
    assert_eq!(fixture.next_started().await.0, "hold second");
    fixture.observed.finish.add_permits(1);
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Result {
            is_error: false,
            ..
        })
    ));
    assert_eq!(fixture.next_started().await.0, "third");
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Result {
            is_error: false,
            ..
        })
    ));
    assert_eq!(fixture.observed.maximum_active.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.supervisor.turn_count(), 3);
    assert_eq!(fixture.supervisor.snapshot().active_turn, None);
    fixture.close().await;
}

#[tokio::test]
async fn cancel_requires_terminal_acknowledgement_before_the_next_turn() {
    let mut fixture = Fixture::start("cancel with delayed ACK").await;
    fixture.next_started().await;
    let active_turn = fixture.supervisor.snapshot().active_turn;
    assert!(active_turn.is_some());
    fixture
        .supervisor
        .send_followup("queued before cancellation")
        .unwrap();
    fixture.supervisor.cancel_turn().unwrap();
    tokio::time::timeout(DEADLINE, fixture.observed.cancellation_seen.notified())
        .await
        .expect("provider receives cancellation");
    let pending = fixture.supervisor.snapshot();
    assert_eq!(pending.active_turn, active_turn);
    assert_eq!(pending.queued_turns, 0);
    assert!(pending.cancellation_requested);
    assert!(pending.last_outcome.is_none());
    assert!(
        fixture
            .supervisor
            .send_followup("before acknowledgement")
            .is_err()
    );
    assert_eq!(fixture.supervisor.turn_count(), 0);
    fixture.observed.acknowledge_cancel.add_permits(1);
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Cancelled { .. })
    ));
    let ended = fixture.supervisor.snapshot();
    assert_eq!(ended.active_turn, None);
    assert_eq!(
        ended.last_outcome.unwrap().stop_reason,
        Some(StopReason::Cancelled)
    );
    fixture
        .supervisor
        .send_followup("after cancellation")
        .unwrap();
    assert_eq!(fixture.next_started().await.0, "after cancellation");
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Result {
            is_error: false,
            ..
        })
    ));
    fixture.close().await;
}

#[tokio::test]
async fn busy_configuration_is_refused_and_does_not_change_the_next_turn_model() {
    let mut fixture = Fixture::start("hold configuration").await;
    fixture.next_started().await;
    assert!(
        fixture
            .supervisor
            .send_config_change("model".into(), ScriptedProvider::FAST.into())
            .is_err()
    );
    assert!(
        fixture
            .supervisor
            .set_config_option(
                "model".into(),
                SessionConfigOptionValue::ValueId {
                    value: ScriptedProvider::FAST.into(),
                },
            )
            .await
            .is_err()
    );
    fixture.observed.finish.add_permits(1);
    fixture.next_terminal().await.unwrap();
    fixture.supervisor.send_followup("model unchanged").unwrap();
    assert_eq!(
        fixture.next_started().await,
        (
            "model unchanged".into(),
            Some(ScriptedProvider::SLOW.into())
        )
    );
    fixture.next_terminal().await.unwrap();
    fixture.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_configuration_errors_are_observable_and_completed_requests_release_admission() {
    let mut fixture = Fixture::start("complete before configuration").await;
    fixture.next_started().await;
    fixture.next_terminal().await.unwrap();
    fixture
        .supervisor
        .send_config_change("missing-config".into(), "missing-value".into())
        .unwrap();
    assert!(fixture.next_terminal().await.is_err());
    assert_eq!(
        fixture.supervisor.snapshot().residency,
        SessionResidency::Resident
    );
    for model in [ScriptedProvider::FAST, ScriptedProvider::SLOW] {
        let options = fixture
            .supervisor
            .set_config_option(
                "model".into(),
                SessionConfigOptionValue::ValueId {
                    value: model.into(),
                },
            )
            .await
            .expect("the previous request releases admission before its reply");
        assert!(
            serde_json::to_value(options)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|option| { option["id"] == "model" && option["currentValue"] == model }),
            "the provider acknowledges the requested model"
        );
    }
    fixture
        .supervisor
        .send_followup("model after configuration")
        .unwrap();
    assert_eq!(
        fixture.next_started().await,
        (
            "model after configuration".into(),
            Some(ScriptedProvider::SLOW.into())
        )
    );
    fixture.next_terminal().await.unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn provider_death_during_a_turn_does_not_fabricate_success_or_cancellation() {
    let mut fixture = Fixture::start("hold until disconnect").await;
    fixture.next_started().await;
    fixture.server.abort();
    assert!(fixture.next_terminal().await.is_err());
    let snapshot = fixture.supervisor.snapshot();
    let outcome = snapshot.last_outcome.expect("interrupted turn observation");
    assert!(outcome.stop_reason.is_none());
    assert!(outcome.error.is_some());
    assert_eq!(snapshot.active_turn, None);
    assert_eq!(fixture.supervisor.turn_count(), 0);
    assert!(
        fixture
            .supervisor
            .send_followup("must not restart")
            .is_err()
    );
}

#[tokio::test]
async fn provider_death_between_turns_preserves_the_last_confirmed_outcome() {
    let mut fixture = Fixture::start("complete before disconnect").await;
    fixture.next_started().await;
    fixture.next_terminal().await.unwrap();
    let confirmed = fixture.supervisor.snapshot().last_outcome.unwrap();
    fixture.server.abort();
    assert!(fixture.next_terminal().await.is_err());
    let observed = fixture.supervisor.snapshot().last_outcome.unwrap();
    assert_eq!(observed.turn_id, confirmed.turn_id);
    assert_eq!(observed.stop_reason, Some(StopReason::EndTurn));
    assert!(observed.error.is_none());
    assert!(
        fixture
            .supervisor
            .send_followup("must not restart")
            .is_err()
    );
}

#[tokio::test]
async fn provider_prompt_failure_is_an_observed_error_without_a_stop_reason() {
    let mut fixture = Fixture::start("fail").await;
    fixture.next_started().await;
    assert!(fixture.next_terminal().await.is_err());
    let outcome = fixture.supervisor.snapshot().last_outcome.unwrap();
    assert!(outcome.stop_reason.is_none());
    assert!(outcome.error.unwrap().contains("controlled turn failure"));
    assert_eq!(fixture.supervisor.turn_count(), 0);
}

#[tokio::test]
async fn an_error_terminal_discards_queued_input_and_retains_its_actual_stop_reason() {
    let mut fixture = Fixture::start("hold max tokens").await;
    fixture.next_started().await;
    fixture
        .supervisor
        .send_followup("queued before token limit")
        .unwrap();
    fixture.observed.finish.add_permits(1);
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Result { is_error: true, .. })
    ));
    let snapshot = fixture.supervisor.snapshot();
    assert_eq!(snapshot.active_turn, None);
    assert_eq!(snapshot.queued_turns, 0);
    assert_eq!(snapshot.residency, SessionResidency::Resident);
    let outcome = snapshot.last_outcome.unwrap();
    assert_eq!(outcome.stop_reason, Some(StopReason::MaxTokens));
    assert!(
        outcome.error.is_none(),
        "the provider sent a confirmed terminal response"
    );
    assert_eq!(fixture.supervisor.session_state(), AcpSessionState::Failed);
    fixture
        .supervisor
        .send_followup("explicit fresh turn")
        .unwrap();
    assert_eq!(fixture.next_started().await.0, "explicit fresh turn");
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Result {
            is_error: false,
            ..
        })
    ));
    fixture.close().await;
}

#[tokio::test]
async fn max_turns_refuses_followups_before_provider_inference() {
    let mut fixture = Fixture::start_bounded(
        "hold limited opening",
        crate::agent::observation::ObservationCapacity::DEFAULT,
        Some(1),
    )
    .await;
    fixture.next_started().await;
    assert!(
        fixture
            .supervisor
            .send_followup("blocked while active")
            .is_err()
    );
    fixture.observed.finish.add_permits(1);
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Result {
            is_error: false,
            ..
        })
    ));
    assert!(
        fixture
            .supervisor
            .send_followup("blocked after completion")
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn detaching_the_observer_keeps_the_turn_resident_until_explicit_termination() {
    let mut fixture = Fixture::start("hold while detached").await;
    fixture.next_started().await;
    let before = fixture.supervisor.snapshot();
    fixture.supervisor.detach_client();
    fixture.messages.close();
    let detached = fixture.supervisor.snapshot();
    assert_eq!(detached.active_turn, before.active_turn);
    assert!(!detached.cancellation_requested);
    assert!(detached.last_outcome.is_none());
    fixture.observed.finish.add_permits(1);
    tokio::time::timeout(DEADLINE, async {
        while fixture.supervisor.snapshot().last_outcome.is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("detached supervisor continues observing the provider");
    let completed = fixture.supervisor.snapshot().last_outcome.unwrap();
    assert_eq!(completed.stop_reason, Some(StopReason::EndTurn));
    fixture.close().await;
    let terminated = fixture.supervisor.snapshot();
    assert_eq!(terminated.active_turn, None);
    assert_eq!(
        terminated.last_outcome.unwrap().stop_reason,
        Some(StopReason::EndTurn)
    );
    assert!(
        fixture
            .supervisor
            .send_followup("already terminated")
            .is_err()
    );
}

#[tokio::test]
async fn losing_the_message_receiver_does_not_end_the_owned_turn() {
    let mut fixture = Fixture::start("hold after observer loss").await;
    fixture.next_started().await;
    fixture.messages.close();
    fixture.observed.finish.add_permits(1);
    tokio::time::timeout(DEADLINE, async {
        while fixture.supervisor.snapshot().last_outcome.is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the owner observes the actual terminal despite receiver loss");
    let snapshot = fixture.supervisor.snapshot();
    assert_eq!(snapshot.residency, SessionResidency::Resident);
    assert!(!snapshot.client_attached);
    assert_eq!(
        snapshot.last_outcome.unwrap().stop_reason,
        Some(StopReason::EndTurn)
    );
    fixture.close().await;
}

#[tokio::test]
async fn confirmed_terminal_facts_survive_owner_abort_and_drop() {
    let mut fixture = Fixture::start("complete before owner cleanup").await;
    fixture.next_started().await;
    fixture.next_terminal().await.unwrap();
    let confirmed = fixture.supervisor.snapshot().last_outcome;
    let retained = Arc::clone(&fixture.supervisor.inner);
    fixture.supervisor.abort();
    assert_eq!(fixture.supervisor.snapshot().last_outcome, confirmed);
    tokio::time::timeout(DEADLINE, &mut fixture.server)
        .await
        .expect("abort closes the provider connection")
        .expect("provider exits");
    drop(fixture);
    assert_eq!(retained.lock().unwrap().snapshot.last_outcome, confirmed);
}

#[tokio::test]
async fn termination_releases_an_active_transport_and_refuses_further_admission() {
    let mut fixture = Fixture::start("hold until termination").await;
    fixture.next_started().await;
    fixture.close().await;
    assert_eq!(fixture.supervisor.snapshot().active_turn, None);
    assert!(fixture.supervisor.send_followup("closed session").is_err());
    fixture.supervisor.terminate_session().await.unwrap();
}

#[tokio::test]
async fn dropping_a_pending_termination_future_retains_the_owned_cleanup_task() {
    let mut fixture = Fixture::start("cancel while termination is interrupted").await;
    fixture.next_started().await;
    {
        let termination = fixture.supervisor.terminate_session();
        tokio::pin!(termination);
        tokio::select! {
            biased;
            result = &mut termination => panic!("cleanup finished before delayed ACK: {result:?}"),
            () = fixture.observed.cancellation_seen.notified() => {}
            () = tokio::time::sleep(DEADLINE) => panic!("provider never saw termination cancellation"),
        }
    }
    assert!(
        fixture.supervisor.task.is_some(),
        "dropping the caller's future must not detach the owner task"
    );
    fixture.observed.acknowledge_cancel.add_permits(1);
    fixture.close().await;
}

#[tokio::test]
async fn dropping_the_owner_closes_the_real_provider_connection() {
    let (client, agent) = agent_client_protocol::Channel::duplex();
    let mut server = tokio::spawn(async move {
        let _ = magistr_acp_agent::serve(ScriptedProvider, agent).await;
    });
    let directory = tempfile::tempdir().unwrap();
    let mut transport = magistr_acp::AcpTransport::new_with_args(
        std::path::PathBuf::from("unused-in-process-agent"),
        Vec::new(),
        Some(directory.path().to_path_buf()),
    )
    .unwrap();
    transport.connect_over(client).await.unwrap();
    let (supervisor, _messages) = SessionSupervisor::start_connected("hang", transport, None)
        .await
        .unwrap();
    drop(supervisor);
    tokio::time::timeout(DEADLINE, &mut server)
        .await
        .expect("owner drop closes the provider connection")
        .expect("provider task ends");
}

struct CountingAllow(Arc<AtomicUsize>);

impl magistr_application::sessions::SessionPolicy for CountingAllow {
    fn decide(
        &mut self,
        _request: &magistr_application::sessions::SessionPolicyRequest,
    ) -> magistr_application::sessions::PolicyDecision {
        self.0.fetch_add(1, Ordering::SeqCst);
        magistr_application::sessions::PolicyDecision::Allow
    }
}

#[tokio::test]
async fn late_permission_gets_cancelled_on_the_wire_and_fresh_turn_can_receive_allow() {
    use agent_client_protocol::schema::ProtocolVersion;
    use agent_client_protocol::schema::v1::{
        CancelNotification, InitializeRequest, InitializeResponse, NewSessionRequest,
        NewSessionResponse, PermissionOption, PermissionOptionKind, PromptRequest, PromptResponse,
        RequestPermissionOutcome, RequestPermissionRequest, ToolCallUpdate, ToolCallUpdateFields,
    };
    use agent_client_protocol::{Agent, on_receive_notification, on_receive_request};

    let (client, agent) = agent_client_protocol::Channel::duplex();
    let cancellation = Arc::new(Notify::new());
    let acknowledge = Arc::new(Semaphore::new(0));
    let (started, mut starts) = mpsc::unbounded_channel();
    let (outcome, mut outcomes) = mpsc::unbounded_channel();
    let mut server = tokio::spawn(
        Agent
            .builder()
            .name("supervisor-permission-test")
            .on_receive_request(
                async move |_request: InitializeRequest, responder, _cx| {
                    responder.respond(InitializeResponse::new(ProtocolVersion::V1))
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async move |_request: NewSessionRequest, responder, _cx| {
                    responder.respond(NewSessionResponse::new("permission-session"))
                },
                on_receive_request!(),
            )
            .on_receive_notification(
                {
                    let cancellation = Arc::clone(&cancellation);
                    async move |_request: CancelNotification, _cx| {
                        cancellation.notify_one();
                        Ok(())
                    }
                },
                on_receive_notification!(),
            )
            .on_receive_request(
                {
                    let cancellation = Arc::clone(&cancellation);
                    let acknowledge = Arc::clone(&acknowledge);
                    async move |request: PromptRequest, responder, cx| {
                        let cancellation = Arc::clone(&cancellation);
                        let acknowledge = Arc::clone(&acknowledge);
                        let started = started.clone();
                        let outcome = outcome.clone();
                        let connection = cx.clone();
                        cx.spawn(async move {
                            let first = request.prompt.iter().any(|block| {
                                matches!(block, agent_client_protocol::schema::v1::ContentBlock::Text(text) if text.text == "first")
                            });
                            let _ = started.send(first);
                            if first {
                                cancellation.notified().await;
                            }
                            // Raw ACP deliberately awaits the actual client reply;
                            // Turn::request_permission would mask it with a local
                            // cancellation result before the wire answer arrives.
                            let response = connection
                                .send_request(RequestPermissionRequest::new(
                                    request.session_id,
                                    ToolCallUpdate::new(
                                        if first { "late" } else { "fresh" },
                                        ToolCallUpdateFields::new().title("Read"),
                                    ),
                                    vec![
                                        PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
                                        PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
                                    ],
                                ))
                                .block_task()
                                .await?;
                            let _ = outcome.send(response.outcome);
                            if first {
                                acknowledge.acquire().await.expect("ACK gate").forget();
                            }
                            responder.respond(PromptResponse::new(if first {
                                StopReason::Cancelled
                            } else {
                                StopReason::EndTurn
                            }))
                        })?;
                        Ok(())
                    }
                },
                on_receive_request!(),
            )
            .connect_to(agent),
    );
    let directory = tempfile::tempdir().unwrap();
    let mut transport = magistr_acp::AcpTransport::new_with_args(
        std::path::PathBuf::from("unused-in-process-agent"),
        Vec::new(),
        Some(directory.path().to_path_buf()),
    )
    .unwrap();
    transport.connect_over(client).await.unwrap();
    let decisions = Arc::new(AtomicUsize::new(0));
    let (mut supervisor, mut messages) = SessionSupervisor::start_connected(
        "first",
        transport,
        Some((
            Box::new(CountingAllow(Arc::clone(&decisions))),
            Default::default(),
        )),
    )
    .await
    .unwrap();
    assert_eq!(
        tokio::time::timeout(DEADLINE, starts.recv()).await.unwrap(),
        Some(true)
    );
    supervisor.cancel_turn().unwrap();
    assert!(matches!(
        tokio::time::timeout(DEADLINE, outcomes.recv())
            .await
            .unwrap(),
        Some(RequestPermissionOutcome::Cancelled)
    ));
    assert_eq!(
        decisions.load(Ordering::SeqCst),
        0,
        "late permission never enters grant policy"
    );
    assert!(supervisor.snapshot().last_outcome.is_none());
    acknowledge.add_permits(1);
    tokio::time::timeout(DEADLINE, async {
        while let Some(message) = messages.recv().await {
            if matches!(message.unwrap(), AgentMessage::Cancelled { .. }) {
                return;
            }
        }
        panic!("cancelled prompt acknowledgement missing");
    })
    .await
    .unwrap();
    supervisor.send_followup("second").unwrap();
    assert_eq!(
        tokio::time::timeout(DEADLINE, starts.recv()).await.unwrap(),
        Some(false)
    );
    assert!(matches!(
        tokio::time::timeout(DEADLINE, outcomes.recv()).await.unwrap(),
        Some(RequestPermissionOutcome::Selected(selected)) if selected.option_id.to_string() == "allow"
    ));
    assert_eq!(decisions.load(Ordering::SeqCst), 1);
    supervisor.terminate_session().await.unwrap();
    tokio::time::timeout(DEADLINE, &mut server)
        .await
        .expect("permission fixture connection closes")
        .expect("permission fixture task exits")
        .expect("permission fixture serves successfully");
}

/// Spin until the owner has observed the turn's terminal — the way the
/// detached-observer tests do — without reading a single message.
async fn until_outcome(fixture: &Fixture) {
    tokio::time::timeout(DEADLINE, async {
        while fixture.supervisor.snapshot().last_outcome.is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the owner observes the provider's terminal");
}

#[tokio::test]
async fn a_flooding_provider_with_a_slow_observer_stays_bounded_and_the_terminal_still_arrives() {
    let mut fixture = Fixture::start("flood text").await;
    fixture.next_started().await;
    // The observer reads nothing until the provider has said everything.
    until_outcome(&fixture).await;
    let cap = crate::agent::observation::ObservationCapacity::DEFAULT.text_bytes;
    assert!(
        fixture.messages.pending_text_bytes() <= cap,
        "pending text {} exceeds the bound {cap}",
        fixture.messages.pending_text_bytes()
    );
    assert!(
        fixture.messages.trimmed_bytes() > 1024 * 1024,
        "most of the flood was trimmed: {}",
        fixture.messages.trimmed_bytes()
    );
    assert!(
        fixture.messages.pending_items() < 16,
        "coalesced deltas, not one item per kilobyte: {}",
        fixture.messages.pending_items()
    );
    let mut lagged = None;
    let mut text_bytes = 0;
    let mut terminal = None;
    while let Some(message) = fixture.messages.recv().await {
        match message {
            Ok(AgentMessage::System { subtype, data }) if subtype == "observation_lagged" => {
                lagged = Some(data["bytes"].as_u64().expect("bytes"));
            }
            Ok(AgentMessage::Assistant { message, .. }) => {
                for block in message.content {
                    if let crate::agent::message::ContentBlock::Text { text } = block {
                        text_bytes += text.len();
                    }
                }
            }
            Ok(AgentMessage::Result { .. }) => {
                terminal = Some(());
                break;
            }
            Err(error) => panic!("the flood must not fail observation: {error}"),
            Ok(_) => {}
        }
    }
    assert!(
        terminal.is_some(),
        "the terminal outcome reaches the observer"
    );
    let lagged = lagged.expect("the gap is visible as a lag marker");
    assert!(
        text_bytes <= cap,
        "delivered text {text_bytes} within the bound"
    );
    assert_eq!(
        lagged as usize + text_bytes,
        2048 * 1024 + "started: flood text".len() + "finished: flood text".len(),
        "trimmed plus delivered accounts for every byte the provider sent"
    );
    assert!(fixture.supervisor.snapshot().client_attached);
    fixture.close().await;
}

#[tokio::test]
async fn a_lossless_backlog_past_its_bound_ends_observation_explicitly_and_keeps_the_session() {
    let mut fixture = Fixture::start_bounded(
        "flood tools",
        crate::agent::observation::ObservationCapacity {
            lossless_items: 8,
            text_bytes: 1024,
        },
        None,
    )
    .await;
    fixture.next_started().await;
    until_outcome(&fixture).await;
    let snapshot = fixture.supervisor.snapshot();
    assert_eq!(
        snapshot.last_outcome.unwrap().stop_reason,
        Some(StopReason::EndTurn),
        "the provider's turn completed regardless of the observer"
    );
    assert_eq!(snapshot.residency, SessionResidency::Resident);
    assert!(
        !snapshot.client_attached,
        "an overflowed observer is a detached one"
    );
    assert!(fixture.messages.has_overflowed());
    let mut last = None;
    while let Some(message) = fixture.messages.recv().await {
        last = Some(message);
    }
    assert!(
        matches!(last, Some(Err(error)) if error.to_string().contains("observation overflowed")),
        "observation ends behind an explicit marker, never silently"
    );
    fixture
        .supervisor
        .send_followup("still resident")
        .expect("the session admits a turn after its observer overflowed");
    assert_eq!(fixture.next_started().await.0, "still resident");
    fixture.close().await;
}

#[tokio::test]
async fn the_turn_queue_has_an_explicit_capacity_and_refuses_the_next_follow_up() {
    let mut fixture = Fixture::start("hold the line").await;
    fixture.next_started().await;
    for index in 0..SessionSupervisor::QUEUED_TURN_CAPACITY {
        fixture
            .supervisor
            .send_followup(&format!("queued {index}"))
            .expect("within capacity");
    }
    let refused = fixture
        .supervisor
        .send_followup("one too many")
        .expect_err("the queue is full");
    assert!(
        refused.to_string().contains("Turn queue is full"),
        "{refused}"
    );
    assert_eq!(
        fixture.supervisor.snapshot().queued_turns,
        SessionSupervisor::QUEUED_TURN_CAPACITY
    );
    fixture.supervisor.cancel_turn().unwrap();
    fixture.next_terminal().await.unwrap();
    assert_eq!(fixture.supervisor.snapshot().queued_turns, 0);
    fixture.close().await;
}

#[tokio::test]
async fn an_interrupt_stops_admission_cancels_the_turn_and_closes_after_the_provider_acknowledges()
{
    let mut fixture = Fixture::start("cancel with delayed ACK").await;
    fixture.next_started().await;
    fixture
        .supervisor
        .send_followup("queued before the interrupt")
        .unwrap();
    fixture.supervisor.interrupt().unwrap();
    let refused = fixture
        .supervisor
        .send_followup("after the interrupt")
        .expect_err("no turn is admitted while draining");
    assert!(refused.to_string().contains("winding down"), "{refused}");
    tokio::time::timeout(DEADLINE, fixture.observed.cancellation_seen.notified())
        .await
        .expect("the provider receives the cancellation");
    let draining = fixture.supervisor.snapshot();
    assert!(draining.cancellation_requested);
    assert_eq!(draining.queued_turns, 0);
    assert_eq!(draining.residency, SessionResidency::Resident);
    assert!(
        draining.last_outcome.is_none(),
        "no outcome is invented before the provider answers"
    );
    fixture.observed.acknowledge_cancel.add_permits(1);
    assert!(matches!(
        fixture.next_terminal().await,
        Ok(AgentMessage::Cancelled { .. })
    ));
    tokio::time::timeout(DEADLINE, &mut fixture.server)
        .await
        .expect("the owner closes the transport once the turn has ended")
        .expect("the provider task ends");
    let ended = fixture.supervisor.snapshot();
    assert_eq!(ended.residency, SessionResidency::Terminated);
    assert_eq!(
        ended.last_outcome.unwrap().stop_reason,
        Some(StopReason::Cancelled)
    );
    fixture.supervisor.terminate_session().await.unwrap();
}

#[tokio::test]
async fn an_interrupt_on_an_idle_session_closes_it_at_once() {
    let mut fixture = Fixture::start("").await;
    fixture.supervisor.interrupt().unwrap();
    tokio::time::timeout(DEADLINE, &mut fixture.server)
        .await
        .expect("an idle owner closes without waiting for anything")
        .expect("the provider task ends");
    assert_eq!(
        fixture.supervisor.snapshot().residency,
        SessionResidency::Terminated
    );
    assert!(fixture.supervisor.send_followup("too late").is_err());
}

#[tokio::test]
async fn empty_opening_keeps_the_resident_session_idle_until_input_arrives() {
    let mut fixture = Fixture::start("").await;
    assert_eq!(fixture.supervisor.snapshot().active_turn, None);
    assert_eq!(fixture.supervisor.turn_count(), 0);
    fixture
        .supervisor
        .send_followup("first explicit turn")
        .unwrap();
    assert_eq!(fixture.next_started().await.0, "first explicit turn");
    fixture.next_terminal().await.unwrap();
    assert_eq!(fixture.supervisor.turn_count(), 1);
    fixture.close().await;
}

/// Decides by the tool title the raw agent copies from the prompt text.
struct TitledPolicy(mpsc::UnboundedSender<String>);

impl magistr_application::sessions::SessionPolicy for TitledPolicy {
    fn decide(
        &mut self,
        request: &magistr_application::sessions::SessionPolicyRequest,
    ) -> magistr_application::sessions::PolicyDecision {
        let _ = self.0.send(format!("decide {}", request.tool_title));
        match request.tool_title.as_str() {
            "allow" => magistr_application::sessions::PolicyDecision::Allow,
            "deny" => magistr_application::sessions::PolicyDecision::Deny,
            _ => magistr_application::sessions::PolicyDecision::Ask(
                magistr_engine::policy::RiskTier::Medium,
            ),
        }
    }

    fn answered(
        &mut self,
        request: &magistr_application::sessions::SessionPolicyRequest,
        kind: magistr_application::sessions::PermissionKind,
    ) {
        let _ = self
            .0
            .send(format!("answered {} {kind:?}", request.tool_title));
    }
}

type WireOutcomes =
    mpsc::UnboundedReceiver<agent_client_protocol::schema::v1::RequestPermissionOutcome>;

/// A raw ACP agent that requests one permission per turn, titled by the
/// prompt text, reports the wire outcome and ends the turn.
async fn permission_agent() -> (
    magistr_acp::AcpTransport,
    tokio::task::JoinHandle<()>,
    WireOutcomes,
    tempfile::TempDir,
) {
    use agent_client_protocol::schema::ProtocolVersion;
    use agent_client_protocol::schema::v1::{
        ContentBlock, InitializeRequest, InitializeResponse, NewSessionRequest, NewSessionResponse,
        PermissionOption, PermissionOptionKind, PromptRequest, PromptResponse,
        RequestPermissionRequest, ToolCallUpdate, ToolCallUpdateFields,
    };
    use agent_client_protocol::{Agent, on_receive_request};

    let (client, agent) = agent_client_protocol::Channel::duplex();
    let (outcome, outcomes) = mpsc::unbounded_channel();
    let server = tokio::spawn(async move {
        let _ = Agent
            .builder()
            .name("supervisor-parity-test")
            .on_receive_request(
                async move |_request: InitializeRequest, responder, _cx| {
                    responder.respond(InitializeResponse::new(ProtocolVersion::V1))
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async move |_request: NewSessionRequest, responder, _cx| {
                    responder.respond(NewSessionResponse::new("parity-session"))
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async move |request: PromptRequest, responder, cx| {
                    let outcome = outcome.clone();
                    let connection = cx.clone();
                    cx.spawn(async move {
                        let title = request
                            .prompt
                            .iter()
                            .find_map(|block| match block {
                                ContentBlock::Text(text) => Some(text.text.clone()),
                                _ => None,
                            })
                            .unwrap_or_default();
                        let response = connection
                            .send_request(RequestPermissionRequest::new(
                                request.session_id,
                                ToolCallUpdate::new(
                                    format!("call-{title}"),
                                    ToolCallUpdateFields::new().title(title),
                                ),
                                vec![
                                    PermissionOption::new(
                                        "allow",
                                        "Allow",
                                        PermissionOptionKind::AllowOnce,
                                    ),
                                    PermissionOption::new(
                                        "reject",
                                        "Reject",
                                        PermissionOptionKind::RejectOnce,
                                    ),
                                ],
                            ))
                            .block_task()
                            .await?;
                        let _ = outcome.send(response.outcome);
                        responder.respond(PromptResponse::new(StopReason::EndTurn))
                    })?;
                    Ok(())
                },
                on_receive_request!(),
            )
            .connect_to(agent)
            .await;
    });
    let directory = tempfile::tempdir().unwrap();
    let mut transport = magistr_acp::AcpTransport::new_with_args(
        std::path::PathBuf::from("unused-in-process-agent"),
        Vec::new(),
        Some(directory.path().to_path_buf()),
    )
    .unwrap();
    transport.connect_over(client).await.unwrap();
    (transport, server, outcomes, directory)
}

fn wire(outcome: agent_client_protocol::schema::v1::RequestPermissionOutcome) -> String {
    match outcome {
        agent_client_protocol::schema::v1::RequestPermissionOutcome::Selected(selected) => {
            format!("selected {}", selected.option_id)
        }
        _ => "cancelled".to_owned(),
    }
}

#[tokio::test]
async fn headless_and_cockpit_attachments_share_policy_decisions_and_terminal_outcomes() {
    // Headless: the resolved policy answers on the owner, Ask is refused.
    let (transport, mut server, mut outcomes, _directory) = permission_agent().await;
    let (policy_log, mut headless_log) = mpsc::unbounded_channel();
    let (mut supervisor, mut messages) = SessionSupervisor::start_connected(
        "allow",
        transport,
        Some((Box::new(TitledPolicy(policy_log)), Default::default())),
    )
    .await
    .unwrap();
    let mut headless = Vec::new();
    for prompt in ["deny", "ask"] {
        let outcome = tokio::time::timeout(DEADLINE, outcomes.recv())
            .await
            .unwrap()
            .unwrap();
        tokio::time::timeout(DEADLINE, async {
            while let Some(message) = messages.recv().await {
                if matches!(message.unwrap(), AgentMessage::Result { .. }) {
                    return;
                }
            }
            panic!("headless terminal missing");
        })
        .await
        .unwrap();
        let snapshot = supervisor.snapshot();
        headless.push((wire(outcome), snapshot.last_outcome.unwrap().stop_reason));
        supervisor.send_followup(prompt).unwrap();
    }
    let outcome = tokio::time::timeout(DEADLINE, outcomes.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        wire(outcome),
        "cancelled",
        "headless Ask is refused, never allowed"
    );
    supervisor.terminate_session().await.unwrap();
    tokio::time::timeout(DEADLINE, &mut server)
        .await
        .unwrap()
        .unwrap();
    let mut headless_decisions = Vec::new();
    while let Ok(entry) = headless_log.try_recv() {
        headless_decisions.push(entry);
    }

    // Cockpit: the same owner evaluates the same policy; Ask reaches the client.
    let (transport, mut server, mut outcomes, _directory) = permission_agent().await;
    let (policy_log, mut cockpit_log) = mpsc::unbounded_channel();
    let (mut supervisor, mut events) = SessionSupervisor::start_interactive(
        transport,
        magistr_application::sessions::SessionOptions::new().opening_prompt("allow"),
        Box::new(TitledPolicy(policy_log)),
    )
    .unwrap();
    let mut cockpit = Vec::new();
    for prompt in ["deny", "ask"] {
        let outcome = tokio::time::timeout(DEADLINE, outcomes.recv())
            .await
            .unwrap()
            .unwrap();
        tokio::time::timeout(DEADLINE, async {
            while let Some(event) = events.recv().await {
                assert!(
                    !matches!(
                        event,
                        magistr_application::sessions::AgentEvent::PermissionRequest { .. }
                    ),
                    "policy-decided requests never reach the cockpit"
                );
                if matches!(
                    event,
                    magistr_application::sessions::AgentEvent::TurnEnded(_)
                ) {
                    return;
                }
            }
            panic!("cockpit terminal missing");
        })
        .await
        .unwrap();
        let snapshot = supervisor.snapshot();
        cockpit.push((wire(outcome), snapshot.last_outcome.unwrap().stop_reason));
        supervisor.send_followup(prompt).unwrap();
    }
    let id = tokio::time::timeout(DEADLINE, async {
        loop {
            if let magistr_application::sessions::AgentEvent::PermissionRequest {
                id, title, ..
            } = events.recv().await.expect("cockpit event stream")
            {
                assert_eq!(title, "ask");
                return id;
            }
        }
    })
    .await
    .unwrap();
    assert!(
        outcomes.try_recv().is_err(),
        "an Ask stays pending on the wire until the client answers"
    );
    supervisor.answer_permission(id, "not offered").unwrap();
    supervisor.answer_permission(id, "allow").unwrap();
    let outcome = tokio::time::timeout(DEADLINE, outcomes.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        wire(outcome),
        "cancelled",
        "an answer that names no offered option revokes the request"
    );
    tokio::time::timeout(DEADLINE, async {
        while let Some(event) = events.recv().await {
            if matches!(
                event,
                magistr_application::sessions::AgentEvent::TurnEnded(_)
            ) {
                return;
            }
        }
        panic!("cockpit terminal missing");
    })
    .await
    .unwrap();
    supervisor.terminate_session().await.unwrap();
    tokio::time::timeout(DEADLINE, &mut server)
        .await
        .unwrap()
        .unwrap();
    let mut cockpit_decisions = Vec::new();
    while let Ok(entry) = cockpit_log.try_recv() {
        cockpit_decisions.push(entry);
    }

    assert_eq!(
        headless,
        [
            ("selected allow".to_owned(), Some(StopReason::EndTurn)),
            ("selected reject".to_owned(), Some(StopReason::EndTurn)),
        ]
    );
    assert_eq!(
        cockpit, headless,
        "both attachments normalize the same outcomes"
    );
    assert_eq!(
        headless_decisions,
        ["decide allow", "decide deny", "decide ask"]
    );
    assert_eq!(
        cockpit_decisions, headless_decisions,
        "the policy sees identical requests on both attachments"
    );
}
