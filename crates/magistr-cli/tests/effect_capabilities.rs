//! Direct ACP effects use the same live project policy as explicit tool asks.

use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    CreateTerminalRequest, InitializeRequest, InitializeResponse, NewSessionRequest,
    NewSessionResponse, PermissionOption, PermissionOptionKind, PromptRequest, PromptResponse,
    ReadTextFileRequest, RequestPermissionOutcome, RequestPermissionRequest, StopReason,
    ToolCallUpdate, ToolCallUpdateFields, ToolKind, WriteTextFileRequest,
};
use agent_client_protocol::{Agent, Channel, on_receive_request};
use magistr::agent::cockpit_session::CockpitSession;
use magistr::application::WorkspaceAdmission;
use magistr::permissions::{PermissionProfile, SessionPermissionPolicy};
use magistr_acp::AcpTransport;
use magistr_application::effects::{EffectFuture, SourceRoute, SourceWriteReceipt, SourceWriter};
use magistr_application::sessions::{AgentEvent, PermissionKind, SessionOptions};
use magistr_cockpit::agent::spawn_session;
use magistr_cockpit::channel::{Commands, Inbox};
use magistr_cockpit::cockpit::{Cmd, Msg};
#[cfg(unix)]
use magistr_engine::policy::{Charter, PolicyCondition, PolicyEffect, PolicyEngine, PolicyRule};

// The process actions are driven only by the Unix terminal tests; the
// dispatch in `run` stays whole so the enum reads the same everywhere.
#[cfg_attr(not(unix), allow(dead_code))]
#[derive(Clone)]
enum Action {
    ExplicitWrite(PathBuf),
    ExplicitProcess,
    Write(PathBuf),
    Read(PathBuf),
    Process(PathBuf),
}

#[derive(Clone, Copy)]
enum Answer {
    Allow,
    RejectAlways,
    Cancel,
}

fn permission_options() -> Vec<PermissionOption> {
    vec![
        PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
        PermissionOption::new("never", "Never", PermissionOptionKind::RejectAlways),
        PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
    ]
}

/// The admitted route's stand-in for the policy tests: lands the bytes at
/// the resolved path so the shared profile can be checked on disk.
struct DiskWriter;

impl SourceWriter for DiskWriter {
    fn write_text(&self, path: &str, content: &str) -> EffectFuture<'_, SourceWriteReceipt> {
        let path = PathBuf::from(path);
        let content = content.to_owned();
        Box::pin(async move {
            std::fs::write(&path, content).expect("disk");
            Ok(SourceWriteReceipt {
                journal_seq: Some(1),
                revision: Some("disk".into()),
                changed: true,
            })
        })
    }
}

async fn run(
    root: &Path,
    policy: SessionPermissionPolicy,
    actions: Vec<Action>,
    answer: Answer,
) -> (Vec<bool>, Vec<String>) {
    run_routed(
        root,
        policy,
        actions,
        answer,
        SourceRoute::Admitted(std::sync::Arc::new(DiskWriter)),
    )
    .await
}

async fn run_routed(
    root: &Path,
    policy: SessionPermissionPolicy,
    actions: Vec<Action>,
    answer: Answer,
    source: SourceRoute,
) -> (Vec<bool>, Vec<String>) {
    tokio::time::timeout(Duration::from_secs(10), async {
        let count = actions.len();
        let (outcomes_tx, mut outcomes_rx) = tokio::sync::mpsc::unbounded_channel();
        let (client_end, agent_end) = Channel::duplex();
        let agent = tokio::spawn(
            Agent
                .builder()
                .on_receive_request(
                    async move |_req: InitializeRequest, responder, _cx| {
                        responder.respond(InitializeResponse::new(ProtocolVersion::V1))
                    },
                    on_receive_request!(),
                )
                .on_receive_request(
                    async move |_req: NewSessionRequest, responder, _cx| {
                        responder.respond(NewSessionResponse::new("effect-session"))
                    },
                    on_receive_request!(),
                )
                .on_receive_request(
                    async move |req: PromptRequest, responder, cx| {
                        let actions = actions.clone();
                        let outcomes_tx = outcomes_tx.clone();
                        let client = cx.clone();
                        cx.spawn(async move {
                            for action in actions {
                                let allowed = match action {
                                    Action::ExplicitWrite(path) => {
                                        let response = client.send_request(RequestPermissionRequest::new(
                                            req.session_id.clone(),
                                            ToolCallUpdate::new("explicit-write", ToolCallUpdateFields::new()
                                                .title("Write".to_owned())
                                                .kind(ToolKind::Edit)
                                                .raw_input(serde_json::json!({"file_path": path, "content": "changed"}))),
                                            permission_options(),
                                        )).block_task().await?;
                                        matches!(response.outcome, RequestPermissionOutcome::Selected(selected)
                                            if selected.option_id.to_string() == "allow")
                                    }
                                    Action::Write(path) => client.send_request(WriteTextFileRequest::new(
                                        req.session_id.clone(), path, "changed",
                                    )).block_task().await.is_ok(),
                                    Action::Read(path) => client.send_request(ReadTextFileRequest::new(
                                        req.session_id.clone(), path,
                                    )).block_task().await.is_ok(),
                                    Action::ExplicitProcess => {
                                        let response = client.send_request(RequestPermissionRequest::new(
                                            req.session_id.clone(),
                                            ToolCallUpdate::new("explicit-process", ToolCallUpdateFields::new()
                                                .title("Bash".to_owned())
                                                .kind(ToolKind::Execute)
                                                .raw_input(serde_json::json!({"command": "sh -c 'printf escaped > spawned.txt'"}))),
                                            permission_options(),
                                        )).block_task().await?;
                                        matches!(response.outcome, RequestPermissionOutcome::Selected(selected)
                                            if selected.option_id.to_string() == "allow")
                                    }
                                    Action::Process(cwd) => client.send_request(CreateTerminalRequest::new(
                                        req.session_id.clone(), "sh",
                                    ).args(vec!["-c".to_owned(), "printf escaped > spawned.txt".to_owned()])
                                        .cwd(cwd)).block_task().await.is_ok(),
                                };
                                let _ = outcomes_tx.send(allowed);
                            }
                            responder.respond(PromptResponse::new(StopReason::EndTurn))
                        })?;
                        Ok(())
                    },
                    on_receive_request!(),
                )
                .connect_to(agent_end),
        );
        let mut transport = AcpTransport::new_with_args(
            PathBuf::from("unused-in-process-agent"), Vec::new(), Some(root.to_path_buf()),
        ).unwrap().local_effects(source);
        transport.connect_over(client_end).await.unwrap();
        let (inbox, mut events) = Inbox::channel();
        let (command_tx, commands) = Commands::channel();
        let session = CockpitSession::start_connected(
            transport, SessionOptions::new().opening_prompt("exercise effects"), Box::new(policy),
        ).expect("the supervised effect session");
        let session = spawn_session(
            Box::new(session), inbox, commands, None, None, None,
        );
        let mut outcomes = Vec::new();
        let mut asks = Vec::new();
        while outcomes.len() < count {
            tokio::select! {
                outcome = outcomes_rx.recv() => outcomes.push(outcome.expect("the agent reports each effect")),
                event = events.recv() => {
                    if let Some(Msg::Agent(AgentEvent::PermissionRequest { id, title, choices, .. })) = event {
                        asks.push(title);
                        if matches!(answer, Answer::Cancel) {
                            command_tx.send(Cmd::Cancel).unwrap();
                            // An answer queued after cancellation cannot revive the effect.
                            let allow = choices.iter().find(|choice| choice.kind == PermissionKind::AllowOnce).unwrap();
                            command_tx.send(Cmd::Permission { id, option_id: allow.option_id.clone() }).unwrap();
                        } else {
                            let kind = match answer {
                                Answer::Allow => PermissionKind::AllowOnce,
                                Answer::RejectAlways => PermissionKind::RejectAlways,
                                Answer::Cancel => unreachable!(),
                            };
                            let choice = choices.iter().find(|choice| choice.kind == kind).unwrap();
                            command_tx.send(Cmd::Permission { id, option_id: choice.option_id.clone() }).unwrap();
                        }
                    }
                }
            }
        }
        drop(command_tx);
        session.await.expect("the session relay joins")
            .expect("the supervised effect session cleanup succeeds");
        agent.abort();
        (outcomes, asks)
    }).await.expect("the effect session settles")
}

fn policy(root: &Path, workflow: bool, protected: Vec<PathBuf>) -> SessionPermissionPolicy {
    SessionPermissionPolicy::new(
        PermissionProfile::Standard,
        Vec::new(),
        Vec::new(),
        protected,
        workflow,
    )
    .rooted_at(Some(root.to_path_buf()))
    .audit_to(None)
}

#[tokio::test]
async fn an_explicit_standing_refusal_denies_the_following_direct_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("target.txt");
    std::fs::write(&path, "original").unwrap();
    let (outcomes, asks) = run(
        dir.path(),
        policy(dir.path(), false, Vec::new()),
        vec![
            Action::ExplicitWrite(path.clone()),
            Action::Write(path.clone()),
        ],
        Answer::RejectAlways,
    )
    .await;
    assert_eq!(outcomes, [false, false]);
    assert_eq!(
        asks,
        ["Write"],
        "the direct effect uses the same live charter"
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "original");
}

#[tokio::test]
async fn protected_direct_writes_are_denied_before_workflow_allowance() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("protected.txt");
    let (outcomes, asks) = run(
        dir.path(),
        policy(dir.path(), true, vec![path.clone()]),
        vec![Action::Write(path.clone())],
        Answer::Allow,
    )
    .await;
    assert_eq!(outcomes, [false]);
    assert!(asks.is_empty());
    assert!(!path.exists());
}

#[tokio::test]
async fn direct_reads_and_approved_writes_use_the_shared_profile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("target.txt");
    std::fs::write(&path, "original").unwrap();
    let (outcomes, asks) = run(
        dir.path(),
        policy(dir.path(), false, Vec::new()),
        vec![Action::Read(path.clone()), Action::Write(path.clone())],
        Answer::Allow,
    )
    .await;
    assert_eq!(outcomes, [true, true]);
    assert_eq!(
        asks,
        ["Write"],
        "the standard profile permits Read and asks for Write"
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "changed");
}

/// A repository root the way a host admits one for its run.
fn repository(tmp: &tempfile::TempDir) -> PathBuf {
    let root = tmp.path().join("workspace");
    std::fs::create_dir_all(root.join(".git")).expect("repository marker");
    std::fs::write(root.join("target.txt"), "original").expect("fixture");
    root
}

async fn admit(root: &Path) -> WorkspaceAdmission {
    WorkspaceAdmission::admit_with(
        magistr_workspace::service::WorkspaceOpenOptions::new(root, env!("CARGO_BIN_EXE_magistr"))
            .with_network_override("0"),
    )
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn an_admitted_workspace_routes_the_approved_write_and_the_session_goes_on() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = repository(&tmp);
    let admission = admit(&root).await;
    let hosted = admission.hosted().expect("this host owns the checkout");
    let root = hosted.service().root().to_path_buf();
    let path = root.join("target.txt");
    let (outcomes, asks) = run_routed(
        &root,
        policy(&root, false, Vec::new()),
        vec![Action::Write(path.clone()), Action::Read(path.clone())],
        Answer::Allow,
        admission.source_route(),
    )
    .await;
    assert_eq!(
        outcomes,
        [true, true],
        "the write lands and the session reads on"
    );
    assert_eq!(
        asks,
        ["Write"],
        "the permission decision precedes the route"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "changed");

    // The owner's word: one journal intent, applied under this generation.
    let session = hosted.service().new_session().expect("session");
    let rows = match hosted
        .service()
        .journal_list(
            &magistr_workspace::service::JournalListInput::default(),
            &session,
        )
        .await
    {
        magistr_workspace::contracts::WorkspaceResponse::Success { results, .. } => {
            results.rows().to_vec()
        }
        magistr_workspace::contracts::WorkspaceResponse::Failure { error, .. } => {
            panic!("journal: {}", error.message)
        }
    };
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].tool, "write");
    let receipt = hosted
        .service()
        .mutation_receipt(rows[0].seq)
        .await
        .expect("read")
        .expect("exists");
    assert_eq!(
        receipt.status,
        magistr_workspace::snapshot::MutationStatus::Applied
    );
    assert_eq!(receipt.generation, Some(hosted.generation()));
    drop(session);
    admission
        .shutdown()
        .await
        .expect("the session released its writer");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_held_workspace_refuses_the_write_without_asking_and_the_read_goes_on() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = repository(&tmp);
    let owner = admit(&root).await;
    let root = owner
        .hosted()
        .expect("owner")
        .service()
        .root()
        .to_path_buf();
    // The second host on the same checkout: refused, never a second owner.
    let second = admit(&root).await;
    assert!(second.hosted().is_none());
    let path = root.join("target.txt");
    let (outcomes, asks) = run_routed(
        &root,
        policy(&root, false, Vec::new()),
        vec![Action::Write(path.clone()), Action::Read(path.clone())],
        Answer::Allow,
        second.source_route(),
    )
    .await;
    assert_eq!(
        outcomes,
        [false, true],
        "the write is refused and the session reads on"
    );
    assert!(
        asks.is_empty(),
        "no permission is asked for a write that cannot land"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "original",
        "nothing lands directly"
    );
    second.shutdown().await.expect("nothing to release");
    owner.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn cancelling_a_direct_write_refuses_its_late_answer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cancelled.txt");
    let (outcomes, asks) = run(
        dir.path(),
        policy(dir.path(), false, Vec::new()),
        vec![Action::Write(path.clone())],
        Answer::Cancel,
    )
    .await;
    assert_eq!(outcomes, [false]);
    assert_eq!(asks, ["Write"]);
    assert!(!path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn a_remembered_process_refusal_prevents_direct_terminal_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "refuse:process",
        PolicyCondition::tool("Bash"),
        PolicyEffect::Deny,
    ));
    let policy = policy(dir.path(), true, Vec::new()).chartered(charter, None);
    let (outcomes, asks) = run(
        dir.path(),
        policy,
        vec![Action::Process(dir.path().to_path_buf())],
        Answer::Allow,
    )
    .await;
    assert_eq!(outcomes, [false]);
    assert!(asks.is_empty());
    assert!(!dir.path().join("spawned.txt").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn an_explicit_program_refusal_denies_the_same_direct_process() {
    let dir = tempfile::tempdir().unwrap();
    let (outcomes, asks) = run(
        dir.path(),
        policy(dir.path(), false, Vec::new()),
        vec![
            Action::ExplicitProcess,
            Action::Process(dir.path().to_path_buf()),
        ],
        Answer::RejectAlways,
    )
    .await;
    assert_eq!(outcomes, [false, false]);
    assert_eq!(
        asks,
        ["Bash"],
        "the original invocation retains its standing refusal"
    );
    assert!(!dir.path().join("spawned.txt").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn governance_over_the_resolved_executable_denies_a_workflow_process() {
    let dir = tempfile::tempdir().unwrap();
    let executable = which::which("sh").unwrap().canonicalize().unwrap();
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "allow:process",
        PolicyCondition::tool("Bash"),
        PolicyEffect::Allow,
    ));
    let governance = PolicyEngine::new(vec![PolicyRule::new(
        "deny:executable",
        PolicyCondition::path(executable.to_str().unwrap()),
        PolicyEffect::Deny,
    )]);
    let policy = policy(dir.path(), true, Vec::new())
        .chartered(charter, None)
        .governed_by(governance);
    let (outcomes, asks) = run(
        dir.path(),
        policy,
        vec![Action::Process(dir.path().to_path_buf())],
        Answer::Allow,
    )
    .await;
    assert_eq!(
        outcomes,
        [false],
        "the executable target is subject to governance"
    );
    assert!(
        asks.is_empty(),
        "governance Deny cannot become an approval prompt"
    );
    assert!(!dir.path().join("spawned.txt").exists());
}
