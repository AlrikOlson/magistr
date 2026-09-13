use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    InitializeRequest, InitializeResponse, NewSessionRequest, NewSessionResponse, SessionId,
    StopReason,
};
use agent_client_protocol::{
    Agent, Channel, Error, JsonRpcRequest, JsonRpcResponse, on_receive_request,
};
use magistr_cockpit::agent::{AgentEvent, SessionOptions, TurnEnd};
use magistr_tui::harness::FakeAcpAgent;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::agent::observation::ObservationReceiver;

use super::{CountingAllow, DEADLINE, SessionResidency, SessionSupervisor};

struct GoalFixture {
    supervisor: SessionSupervisor,
    events: ObservationReceiver<AgentEvent>,
    session_id: String,
    server: JoinHandle<Result<(), Error>>,
    _directory: tempfile::TempDir,
}

impl GoalFixture {
    async fn connect(client: Channel, server: JoinHandle<Result<(), Error>>) -> Self {
        let directory = tempfile::tempdir().expect("goal session directory");
        let mut transport = magistr_acp::AcpTransport::new_with_args(
            PathBuf::from("unused-in-process-agent"),
            Vec::new(),
            Some(directory.path().to_path_buf()),
        )
        .expect("goal transport");
        transport.connect_over(client).await.expect("ACP handshake");
        let (supervisor, mut events) = SessionSupervisor::start_interactive(
            transport,
            SessionOptions::new(),
            Box::new(CountingAllow(Arc::new(AtomicUsize::new(0)))),
        )
        .expect("goal supervisor");
        let session_id = tokio::time::timeout(DEADLINE, async {
            loop {
                match events.recv().await.expect("supervisor initialization") {
                    AgentEvent::Initialized { session_id, .. } => return session_id,
                    AgentEvent::Error(error) => panic!("initialization failed: {error}"),
                    _ => {}
                }
            }
        })
        .await
        .expect("session initialized before goal admission");
        Self {
            supervisor,
            events,
            session_id,
            server,
            _directory: directory,
        }
    }

    async fn close(&mut self) {
        tokio::time::timeout(DEADLINE, self.supervisor.terminate_session())
            .await
            .expect("goal cleanup finishes")
            .expect("goal cleanup succeeds");
        tokio::time::timeout(DEADLINE, &mut self.server)
            .await
            .expect("goal connection closes")
            .expect("goal provider task exits")
            .expect("goal provider serves successfully");
    }
}

impl Drop for GoalFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[tokio::test]
async fn goal_reply_is_observed_after_text_on_both_sides_of_its_echo() {
    let (client, observed, server) = FakeAcpAgent::new().advertising_goal_extension().spawn();
    let mut fixture = GoalFixture::connect(client, server).await;
    for (index, objective) in ["first goal", "next goal"].into_iter().enumerate() {
        let response = fixture
            .supervisor
            .control()
            .begin_goal(
                "_session/goal".into(),
                serde_json::json!({
                    "sessionId": fixture.session_id,
                    "action": "set",
                    "objective": objective,
                }),
            )
            .expect("idle goal admitted");
        let reply = tokio::time::timeout(DEADLINE, response)
            .await
            .expect("goal reply arrives")
            .expect("goal reply retained")
            .expect("goal extension succeeds");
        assert_eq!(reply, serde_json::json!({}));

        // The reply certifies owner observation, so the complete transcript
        // must already be queued. Waiting here could hide an early reply.
        let mut transcript = Vec::new();
        while let Ok(event) = fixture.events.try_recv() {
            transcript.push(event);
        }
        assert_eq!(
            transcript,
            [
                AgentEvent::Text(format!("working toward: {objective}")),
                AgentEvent::System {
                    subtype: "session_info".into(),
                },
                AgentEvent::Text("\ngoal work finished".into()),
                AgentEvent::TurnEnded(TurnEnd::EndTurn),
            ],
            "every pre-response update precedes the owner's terminal observation"
        );
        let snapshot = fixture.supervisor.snapshot();
        assert_eq!(snapshot.active_turn, None);
        let outcome = snapshot.last_outcome.expect("confirmed goal outcome");
        assert_eq!(outcome.turn_id, (index + 1) as u64);
        assert_eq!(outcome.stop_reason, Some(StopReason::EndTurn));
        assert!(outcome.error.is_none());
        assert_eq!(fixture.supervisor.turn_count(), (index + 1) as u32);
    }
    assert_eq!(
        observed.goals(),
        [
            ("set".into(), Some("first goal".into())),
            ("set".into(), Some("next goal".into())),
        ]
    );
    fixture.close().await;
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "_session/goal", response = GoalResponse)]
#[serde(rename_all = "camelCase")]
struct GoalRequest {
    session_id: SessionId,
    action: String,
    objective: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
struct GoalResponse {}

fn rejecting_goal_agent() -> (Channel, JoinHandle<Result<(), Error>>) {
    let (client, agent) = Channel::duplex();
    let server = tokio::spawn(
        Agent
            .builder()
            .name("supervisor-goal-rejection")
            .on_receive_request(
                async move |_request: InitializeRequest, responder, _cx| {
                    responder.respond(InitializeResponse::new(ProtocolVersion::V1))
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async move |_request: NewSessionRequest, responder, _cx| {
                    responder.respond(NewSessionResponse::new("rejected-goal-session"))
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async move |_request: GoalRequest, responder, _cx| {
                    responder.respond_with_internal_error("controlled goal rejection")
                },
                on_receive_request!(),
            )
            .connect_to(agent),
    );
    (client, server)
}

#[tokio::test]
async fn rejected_goal_extension_never_emits_a_successful_terminal() {
    let (client, server) = rejecting_goal_agent();
    let mut fixture = GoalFixture::connect(client, server).await;
    let response = fixture
        .supervisor
        .control()
        .begin_goal(
            "_session/goal".into(),
            serde_json::json!({
                "sessionId": fixture.session_id,
                "action": "set",
                "objective": "rejected work",
            }),
        )
        .expect("goal admitted before provider refusal");
    let error = tokio::time::timeout(DEADLINE, response)
        .await
        .expect("goal rejection arrives")
        .expect("goal reply retained")
        .expect_err("provider refusal must reach the caller");
    assert!(error.to_string().contains("controlled goal rejection"));
    let transcript = tokio::time::timeout(DEADLINE, async {
        let mut transcript = Vec::new();
        while let Some(event) = fixture.events.recv().await {
            transcript.push(event);
        }
        transcript
    })
    .await
    .expect("failed goal closes owner observation");
    assert!(transcript.iter().any(|event| {
        matches!(event, AgentEvent::Error(error) if error.contains("controlled goal rejection"))
    }));
    assert!(
        !transcript
            .iter()
            .any(|event| matches!(event, AgentEvent::TurnEnded(TurnEnd::EndTurn))),
        "a refused extension cannot synthesize a successful terminal: {transcript:?}"
    );
    let snapshot = fixture.supervisor.snapshot();
    assert_eq!(snapshot.active_turn, None);
    assert_eq!(snapshot.residency, SessionResidency::Failed);
    assert_eq!(fixture.supervisor.turn_count(), 0);
    let outcome = snapshot.last_outcome.expect("failed goal observation");
    assert!(outcome.stop_reason.is_none());
    assert!(outcome.error.unwrap().contains("controlled goal rejection"));
    fixture.close().await;
}
