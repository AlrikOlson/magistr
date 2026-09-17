//! The cockpit projection observes the resident supervisor's real ACP turn order.
//! Semaphores control completion and cancellation acknowledgement without timing
//! assumptions about how quickly the owner or provider is scheduled.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use agent_client_protocol::BoxFuture;
use agent_client_protocol::schema::v1::StopReason;
use magistr::agent::cockpit_session::CockpitSession;
use magistr_acp::AcpTransport;
use magistr_acp_agent::{
    AgentError, ModelChoice, Provider, ProviderCapabilities, ScriptedProvider, SessionOpened,
    SessionStart, Turn,
};
use magistr_application::sessions::SessionHandle;
use magistr_application::sessions::{AgentEvent, AskEverything, SessionOptions, TurnEnd};
use tokio::sync::{Semaphore, mpsc};

const DEADLINE: Duration = Duration::from_secs(3);

struct Observed {
    starts: mpsc::UnboundedSender<String>,
    finish: Semaphore,
    acknowledge_cancel: Semaphore,
    active: AtomicUsize,
    maximum_active: AtomicUsize,
}

struct ActiveTurn(Arc<Observed>);

impl Drop for ActiveTurn {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

struct ControlledProvider(Arc<Observed>);

impl Provider for ControlledProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            terminal_cancellation_ack: true,
            ..ProviderCapabilities::text_session()
        }
    }

    fn name(&self) -> &str {
        "cockpit-session-contract"
    }

    fn models(&self) -> Vec<ModelChoice> {
        ScriptedProvider.models()
    }

    fn start(&self, session: SessionStart) -> BoxFuture<'_, Result<SessionOpened, AgentError>> {
        ScriptedProvider.start(session)
    }

    fn prompt(&self, turn: Turn) -> BoxFuture<'_, Result<StopReason, AgentError>> {
        let observed = Arc::clone(&self.0);
        Box::pin(async move {
            let active = observed.active.fetch_add(1, Ordering::SeqCst) + 1;
            observed.maximum_active.fetch_max(active, Ordering::SeqCst);
            let _active = ActiveTurn(Arc::clone(&observed));
            let text = turn.text();
            let _ = observed.starts.send(text.clone());
            turn.message(format!("started: {text}"))?;
            if text == "cancel with delayed acknowledgement" {
                turn.cancelled().await;
                turn.message("cancellation received, awaiting acknowledgement")?;
                observed
                    .acknowledge_cancel
                    .acquire()
                    .await
                    .expect("the cancellation acknowledgement gate")
                    .forget();
                turn.message("last update before acknowledgement")?;
                return Ok(StopReason::Cancelled);
            }
            if text.starts_with("hold") {
                tokio::select! {
                    permit = observed.finish.acquire() => permit.expect("the finish gate").forget(),
                    () = turn.cancelled() => return Ok(StopReason::Cancelled),
                }
            }
            turn.message(format!("finished: {text}"))?;
            Ok(StopReason::EndTurn)
        })
    }
}

struct Fixture {
    session: CockpitSession,
    starts: mpsc::UnboundedReceiver<String>,
    observed: Arc<Observed>,
    events: Vec<AgentEvent>,
    server: tokio::task::JoinHandle<()>,
    _directory: tempfile::TempDir,
}

impl Fixture {
    async fn start(prompt: &str) -> Self {
        let (client, agent) = agent_client_protocol::Channel::duplex();
        let (starts, receiver) = mpsc::unbounded_channel();
        let observed = Arc::new(Observed {
            starts,
            finish: Semaphore::new(0),
            acknowledge_cancel: Semaphore::new(0),
            active: AtomicUsize::new(0),
            maximum_active: AtomicUsize::new(0),
        });
        let provider = ControlledProvider(Arc::clone(&observed));
        let server = tokio::spawn(async move {
            let _ = magistr_acp_agent::serve(provider, agent).await;
        });
        let directory = tempfile::tempdir().expect("the session directory");
        let mut transport = AcpTransport::new_with_args(
            PathBuf::from("unused-in-process-agent"),
            Vec::new(),
            Some(directory.path().to_path_buf()),
        )
        .expect("the transport");
        transport
            .connect_over(client)
            .await
            .expect("the ACP handshake");
        let session = CockpitSession::start_connected(
            transport,
            SessionOptions::new().opening_prompt(prompt),
            Box::new(AskEverything),
        )
        .expect("the supervised cockpit session");
        Self {
            session,
            starts: receiver,
            observed,
            events: Vec::new(),
            server,
            _directory: directory,
        }
    }

    async fn next_started(&mut self) -> String {
        tokio::time::timeout(DEADLINE, self.starts.recv())
            .await
            .expect("the provider starts an admitted prompt")
            .expect("the provider observation channel remains open")
    }

    async fn until(&mut self, predicate: impl Fn(&AgentEvent) -> bool) -> AgentEvent {
        tokio::time::timeout(DEADLINE, async {
            loop {
                let event = self
                    .session
                    .next_event()
                    .await
                    .expect("the cockpit event stream");
                let done = predicate(&event);
                self.events.push(event.clone());
                if done {
                    return event;
                }
            }
        })
        .await
        .expect("the expected cockpit event arrives")
    }

    async fn terminal(&mut self) -> AgentEvent {
        self.until(|event| matches!(event, AgentEvent::TurnEnded(_) | AgentEvent::Error(_)))
            .await
    }

    async fn close(&mut self) {
        tokio::time::timeout(DEADLINE, self.session.terminate_session())
            .await
            .expect("supervisor cleanup finishes")
            .expect("supervisor cleanup succeeds");
        tokio::time::timeout(DEADLINE, &mut self.server)
            .await
            .expect("cleanup closes the real ACP connection")
            .expect("the provider task exits");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[tokio::test]
async fn queued_cockpit_turns_reach_the_provider_in_fifo_order_with_ordered_transcript_events() {
    let mut fixture = Fixture::start("hold first").await;
    assert_eq!(fixture.next_started().await, "hold first");
    fixture.session.submit_turn("hold second").unwrap();
    fixture.session.submit_turn("third").unwrap();
    assert_eq!(fixture.session.snapshot().queued_turns, 2);
    for next in ["hold second", "third"] {
        fixture.observed.finish.add_permits(1);
        assert_eq!(
            fixture.terminal().await,
            AgentEvent::TurnEnded(TurnEnd::EndTurn)
        );
        assert_eq!(fixture.next_started().await, next);
    }
    assert_eq!(
        fixture.terminal().await,
        AgentEvent::TurnEnded(TurnEnd::EndTurn)
    );
    assert_eq!(fixture.observed.maximum_active.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.session.snapshot().active_turn, None);
    assert_eq!(fixture.session.snapshot().queued_turns, 0);
    // Adjacent text deltas may reach the cockpit coalesced when it was a
    // step behind the owner (the bounded observation channel merges them),
    // so the transcript is compared as the text each turn said, not as
    // the chunks it arrived in.
    let mut transcript: Vec<String> = Vec::new();
    for event in &fixture.events {
        match event {
            AgentEvent::UserText(text) => transcript.push(format!("you: {}", text.trim_end())),
            AgentEvent::Text(text) => match transcript.last_mut() {
                Some(last) if last.starts_with("agent: ") => last.push_str(text),
                _ => transcript.push(format!("agent: {text}")),
            },
            AgentEvent::TurnEnded(reason) => transcript.push(format!("ended: {reason:?}")),
            AgentEvent::Error(error) => panic!("queued turns must not be dropped: {error}"),
            _ => {}
        }
    }
    assert_eq!(
        transcript,
        [
            "you: hold first",
            "agent: started: hold firstfinished: hold first",
            "ended: EndTurn",
            "agent: started: hold secondfinished: hold second",
            "ended: EndTurn",
            "agent: started: thirdfinished: third",
            "ended: EndTurn",
        ]
    );
    fixture.close().await;
}

#[tokio::test]
async fn cancellation_keeps_the_cockpit_turn_active_until_the_provider_acknowledges_it() {
    let mut fixture = Fixture::start("cancel with delayed acknowledgement").await;
    fixture.next_started().await;
    let active = fixture.session.snapshot().active_turn;
    fixture
        .session
        .submit_turn("discard this queued input")
        .unwrap();
    fixture.session.cancel_turn().unwrap();
    fixture
        .until(|event| {
            matches!(event, AgentEvent::Text(text)
        if text == "cancellation received, awaiting acknowledgement")
        })
        .await;
    let pending = fixture.session.snapshot();
    assert_eq!(pending.active_turn, active);
    assert!(pending.cancellation_requested);
    assert_eq!(pending.queued_turns, 0);
    assert!(pending.last_outcome.is_none());
    assert!(
        !fixture
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::TurnEnded(_)))
    );
    assert!(
        fixture
            .session
            .submit_turn("before acknowledgement")
            .is_err()
    );

    fixture.observed.acknowledge_cancel.add_permits(1);
    assert_eq!(
        fixture.terminal().await,
        AgentEvent::TurnEnded(TurnEnd::Cancelled)
    );
    let final_events = &fixture.events[fixture.events.len() - 2..];
    assert_eq!(
        final_events,
        [
            AgentEvent::Text("last update before acknowledgement".to_owned()),
            AgentEvent::TurnEnded(TurnEnd::Cancelled),
        ]
    );
    let ended = fixture.session.snapshot();
    assert_eq!(ended.active_turn, None);
    assert!(!ended.cancellation_requested);
    assert_eq!(
        ended.last_outcome.unwrap().stop_reason,
        Some(StopReason::Cancelled)
    );
    fixture
        .session
        .submit_turn("after acknowledgement")
        .unwrap();
    assert_eq!(fixture.next_started().await, "after acknowledgement");
    assert_eq!(
        fixture.terminal().await,
        AgentEvent::TurnEnded(TurnEnd::EndTurn)
    );
    fixture.close().await;
}

#[tokio::test]
async fn a_provider_disconnect_reaches_the_cockpit_as_an_error_without_a_fabricated_terminal() {
    let mut fixture = Fixture::start("hold until disconnect").await;
    fixture.next_started().await;
    fixture.server.abort();
    assert!(matches!(fixture.terminal().await, AgentEvent::Error(_)));
    let outcome = fixture
        .session
        .snapshot()
        .last_outcome
        .expect("the interrupted turn");
    assert!(outcome.stop_reason.is_none());
    assert!(outcome.error.is_some());
    assert!(
        !fixture
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::TurnEnded(_)))
    );
    assert!(fixture.session.submit_turn("must not restart").is_err());
}

#[tokio::test]
async fn detaching_the_cockpit_does_not_cancel_the_resident_provider_turn() {
    let mut fixture = Fixture::start("hold while detached").await;
    fixture.next_started().await;
    let active = fixture.session.snapshot().active_turn;
    fixture.session.detach_client();
    assert!(
        tokio::time::timeout(DEADLINE, fixture.session.next_event())
            .await
            .expect("detaching closes cockpit observation")
            .is_none()
    );
    let detached = fixture.session.snapshot();
    assert_eq!(detached.active_turn, active);
    assert!(!detached.client_attached);
    assert!(!detached.cancellation_requested);
    assert!(detached.last_outcome.is_none());
    fixture.observed.finish.add_permits(1);
    tokio::time::timeout(DEADLINE, async {
        while fixture.session.snapshot().last_outcome.is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the resident owner observes completion after the cockpit detaches");
    assert_eq!(
        fixture.session.snapshot().last_outcome.unwrap().stop_reason,
        Some(StopReason::EndTurn)
    );
    fixture.close().await;
}
