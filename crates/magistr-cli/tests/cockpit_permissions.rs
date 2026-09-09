//! The cockpit permission policy end to end:
//! the real `CockpitPolicy` between the fake ACP agent and the cockpit,
//! one test per branch — a request the profile allows is answered
//! without a strip, a protected-path write is rejected without a strip,
//! workflow mode auto-approves the standard development operations, and
//! everything else still reaches the strip.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::schema::v1::ToolKind;
use magistr::permissions::{AuditEntry, CockpitPolicy, PermissionProfile};
use magistr_acp::AcpTransport;
use magistr_cockpit::agent::{SessionOptions, SessionPolicy, spawn_session};
use magistr_cockpit::cockpit::{Model, Size};
use magistr_tui::graphics::Env;
use magistr_tui::harness::{AcpObserved, FakeAcpAgent, FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};

const MS: fn(u64) -> Duration = Duration::from_millis;
const SIZE: Size = Size::new(80, 24);

/// Connect the transport to `agent` over a duplex channel and run the
/// cockpit with `policy` between the agent's requests and the strip.
async fn run(
    script: Script,
    agent: FakeAcpAgent,
    policy: CockpitPolicy,
) -> (Model, Arc<AcpObserved>) {
    let (client_end, observed, agent_task) = agent.spawn();
    let mut transport =
        AcpTransport::new_with_args(PathBuf::from("fake-acp-agent"), Vec::new(), None)
            .expect("a transport");
    transport
        .connect_over(client_end)
        .await
        .expect("the handshake with the fake agent");
    let terminal = FakeTerminal::new(SIZE, script);
    let mut runtime = Runtime::new(
        terminal,
        Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500)),
    );
    let commands = runtime.commands().expect("the commands, once");
    let session = spawn_session(
        transport,
        runtime.inbox(),
        commands,
        SessionOptions::new().opening_prompt("do the thing"),
        Box::new(policy) as Box<dyn SessionPolicy>,
        None,
        None,
        None,
    );
    let model = runtime
        .run(Model::new(SIZE))
        .await
        .expect("the run succeeds");
    session.await.expect("the session ends with the runtime");
    agent_task.abort();
    (model, observed)
}

/// A quiet script: start up, let the turn run, quit.
fn script() -> Script {
    Script::new().startup_replies(MS(0)).text(MS(600), "\x04")
}

fn audit_entries(path: &std::path::Path) -> Vec<AuditEntry> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("an NDJSON audit entry"))
        .collect()
}

#[tokio::test]
async fn a_request_the_profile_allows_is_answered_without_a_strip() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let log = dir.path().join("permissions.log");
    let policy = CockpitPolicy::new(
        PermissionProfile::Standard,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .audit_to(Some(log.clone()));
    let (model, observed) = run(
        script(),
        FakeAcpAgent::new()
            .asking_permission("Read")
            .with_input(serde_json::json!({"file_path": "/src/main.rs"})),
        policy,
    )
    .await;
    assert_eq!(observed.outcomes(), [Some("allow".to_owned())]);
    assert_eq!(model.permissions_seen(), 0, "no strip was shown");
    let entries = audit_entries(&log);
    assert_eq!(entries.len(), 1, "the profile's decision was audited");
    assert_eq!(entries[0].tool_name, "Read");
    assert!(entries[0].allowed);
}

#[tokio::test]
async fn a_protected_path_write_is_rejected_without_a_strip() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let log = dir.path().join("permissions.log");
    let policy = CockpitPolicy::new(
        PermissionProfile::Yolo,
        Vec::new(),
        Vec::new(),
        vec![PathBuf::from("/project/.magistr/roadmap.yaml")],
        false,
    )
    .audit_to(Some(log.clone()));
    let (model, observed) = run(
        script(),
        FakeAcpAgent::new()
            .asking_permission("Write")
            .with_input(serde_json::json!({"file_path": "/project/.magistr/roadmap.yaml"})),
        policy,
    )
    .await;
    assert_eq!(
        observed.outcomes(),
        [Some("reject".to_owned())],
        "even yolo never writes a protected file"
    );
    assert_eq!(model.permissions_seen(), 0, "no strip was shown");
    assert!(audit_entries(&log).is_empty(), "blocks are not audited");
}

#[tokio::test]
async fn workflow_mode_auto_approves_read_edit_and_execute() {
    for kind in [ToolKind::Read, ToolKind::Edit, ToolKind::Execute] {
        let policy = CockpitPolicy::new(
            PermissionProfile::Cautious,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            true,
        )
        .audit_to(None);
        let (model, observed) = run(
            script(),
            FakeAcpAgent::new()
                .asking_permission("a workflow tool")
                .with_kind(kind),
            policy,
        )
        .await;
        assert_eq!(
            observed.outcomes(),
            [Some("allow".to_owned())],
            "{kind:?} is auto-approved in workflow mode"
        );
        assert_eq!(model.permissions_seen(), 0, "no strip for {kind:?}");
    }
}

#[tokio::test]
async fn a_request_the_policy_cannot_decide_still_reaches_the_strip() {
    let policy = CockpitPolicy::new(
        PermissionProfile::Cautious,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .audit_to(None);
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "a")
        .text(MS(600), "\x04");
    let (model, observed) = run(
        script,
        FakeAcpAgent::new().asking_permission("Frobnicate the flux"),
        policy,
    )
    .await;
    assert_eq!(
        observed.outcomes(),
        [Some("allow".to_owned())],
        "the user's `A` answered the strip"
    );
    assert_eq!(model.permissions_seen(), 1, "the strip was shown");
}
