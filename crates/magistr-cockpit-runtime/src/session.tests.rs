use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use super::*;

use magistr_application::sessions::{PromptAttachment, PromptInput, ToolRecord, TurnEnd};
use magistr_cockpit::cockpit::{WorkspaceCmd, WorkspaceEvent};
use magistr_cockpit::harness::FakeWorkspace;

struct FakeSession {
    events: mpsc::UnboundedReceiver<AgentEvent>,
    calls: mpsc::UnboundedSender<String>,
    cleanup: Option<oneshot::Receiver<()>>,
    cleanup_error: Option<String>,
}

impl SessionHandle for FakeSession {
    fn conversation(&self) -> Option<magistr_application::sessions::Conversation> {
        Some(magistr_application::sessions::Conversation {
            id: "logical-conversation".into(),
            name: None,
            provider: "fake".into(),
            purpose: magistr_application::sessions::SessionPurpose::Conversation,
        })
    }
    fn submit_turn(
        &mut self,
        input: &magistr_application::sessions::PromptInput,
    ) -> Result<(), String> {
        let text = &input.text;
        self.calls
            .send(format!("prompt {text}"))
            .expect("call observer");
        if text == "refused" {
            Err("turn admission refused".to_owned())
        } else {
            Ok(())
        }
    }

    fn rename(&mut self, _: &str) -> Result<(), String> {
        Err("Naming is unavailable in this session.".into())
    }
    fn configure(&mut self, _: &str, _: &str) -> Result<(), String> {
        Err("Agent choices are unavailable in this session.".into())
    }
    fn cancel_turn(&mut self) -> Result<(), String> {
        self.calls.send("cancel".to_owned()).expect("call observer");
        Ok(())
    }

    fn answer_permission(&mut self, id: u64, option_id: &str) -> Result<(), String> {
        self.calls
            .send(format!("answer {id} {option_id}"))
            .expect("call observer");
        Ok(())
    }

    fn constrain_permission(&mut self, id: u64, condition: &str) -> Result<(), String> {
        self.calls
            .send(format!("constrain {id} {condition}"))
            .expect("call observer");
        Ok(())
    }

    fn delegate_permission(&mut self, key: &str) -> Result<(), String> {
        self.calls
            .send(format!("delegate {key}"))
            .expect("call observer");
        Ok(())
    }

    fn kill_terminal(&mut self, id: &str) -> Result<(), String> {
        self.calls
            .send(format!("kill {id}"))
            .expect("call observer");
        Ok(())
    }

    fn detach_client(&mut self) {
        self.calls.send("detach".to_owned()).expect("call observer");
    }

    fn next_event(&mut self) -> Pin<Box<dyn Future<Output = Option<AgentEvent>> + Send + '_>> {
        Box::pin(self.events.recv())
    }

    fn terminate_session(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            self.calls
                .send("terminating".to_owned())
                .expect("call observer");
            if let Some(cleanup) = self.cleanup.take() {
                cleanup.await.expect("cleanup released");
            }
            self.calls
                .send("terminated".to_owned())
                .expect("call observer");
            self.cleanup_error.take().map_or(Ok(()), Err)
        })
    }
}

fn fake() -> (
    FakeSession,
    mpsc::UnboundedSender<AgentEvent>,
    mpsc::UnboundedReceiver<String>,
) {
    let (events_tx, events) = mpsc::unbounded_channel();
    let (calls, calls_rx) = mpsc::unbounded_channel();
    (
        FakeSession {
            events,
            calls,
            cleanup: None,
            cleanup_error: None,
        },
        events_tx,
        calls_rx,
    )
}

async fn next<T>(receiver: &mut mpsc::UnboundedReceiver<T>) -> T {
    tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .expect("relay made progress")
        .expect("event receiver remains open")
}

#[tokio::test]
async fn publishes_accepted_input_once_and_preserves_provider_history_and_permission_ids() {
    let (handle, events, mut calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let task = spawn_session(Box::new(handle), inbox, commands, None, None, None);
    let reported = [
        AgentEvent::Initialized {
            session_id: "retained".to_owned(),
            model: Some("fast".to_owned()),
        },
        AgentEvent::UserText("opening\n".to_owned()),
        AgentEvent::PermissionCancelled { id: 71 },
        AgentEvent::Text("answer".to_owned()),
        AgentEvent::TurnEnded(TurnEnd::Cancelled),
    ];
    for event in &reported {
        events.send(event.clone()).expect("session event sent");
    }
    for event in reported {
        assert!(matches!(next(&mut messages).await, Msg::Agent(actual) if actual == event));
    }
    let submitted = PromptInput {
        text: "follow-up".into(),
        attachments: vec![
            PromptAttachment::from_bytes(
                "context.txt",
                "accepted-hash",
                b"exact\r\ncontext".to_vec(),
            )
            .unwrap(),
        ],
    };
    commands_tx
        .send(Cmd::Prompt(submitted.clone()))
        .expect("command sent");
    assert_eq!(next(&mut calls).await, "prompt follow-up");
    assert!(
        matches!(next(&mut messages).await, Msg::InputAccepted(receipt) if receipt == submitted.receipt())
    );
    events
        .send(AgentEvent::UserText("follow-up\n".into()))
        .unwrap();
    events.send(AgentEvent::Text("next reply".into())).unwrap();
    assert!(
        matches!(next(&mut messages).await, Msg::Agent(AgentEvent::Text(text)) if text == "next reply")
    );
    drop(commands_tx);
    task.await
        .expect("relay task joins")
        .expect("session cleanup succeeds");
}

#[tokio::test]
async fn relay_rejects_commands_for_another_logical_conversation() {
    let (handle, _events, mut calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (sender, commands) = Commands::channel();
    let task = spawn_session(Box::new(handle), inbox, commands, None, None, None);
    let permission = Cmd::Permission {
        id: 8,
        option_id: "allow".into(),
    };
    sender
        .send(Cmd::ForConversation {
            id: "other-conversation".into(),
            command: Box::new(permission.clone()),
        })
        .unwrap();
    assert!(
        matches!(next(&mut messages).await, Msg::CommandFailed { cmd, .. } if cmd == permission)
    );
    assert!(calls.try_recv().is_err());
    sender
        .send(Cmd::ForConversation {
            id: "other-conversation".into(),
            command: Box::new(Cmd::Prompt("stale draft".into())),
        })
        .unwrap();
    assert!(
        matches!(next(&mut messages).await, Msg::Agent(AgentEvent::PromptRejected { input, .. }) if input.text == "stale draft")
    );
    assert!(calls.try_recv().is_err());
    sender
        .send(Cmd::ForConversation {
            id: "logical-conversation".into(),
            command: Box::new(permission),
        })
        .unwrap();
    assert_eq!(next(&mut calls).await, "answer 8 allow");
    drop(sender);
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn routes_commands_to_the_owner_and_reports_admission_errors() {
    let (handle, _events, mut calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let task = spawn_session(Box::new(handle), inbox, commands, None, None, None);
    let submitted = PromptInput {
        text: "refused".into(),
        attachments: vec![
            PromptAttachment::from_bytes(
                "context.txt",
                "refused-hash",
                "exact\r\n日本語".as_bytes().to_vec(),
            )
            .unwrap(),
        ],
    };
    for cmd in [
        Cmd::Constraint {
            id: 8,
            condition: "only this file".to_owned(),
        },
        Cmd::Permission {
            id: 8,
            option_id: "allow".to_owned(),
        },
        Cmd::Delegate("read".to_owned()),
        Cmd::Kill(KillCmd::Terminal("terminal-2".to_owned())),
        Cmd::Cancel,
        Cmd::Prompt(submitted.clone()),
    ] {
        commands_tx.send(cmd).expect("command sent");
    }
    for expected in [
        "constrain 8 only this file",
        "answer 8 allow",
        "delegate read",
        "kill terminal-2",
        "cancel",
        "prompt refused",
    ] {
        assert_eq!(next(&mut calls).await, expected);
    }
    assert!(
        matches!(next(&mut messages).await, Msg::Agent(AgentEvent::PromptRejected { input, error }) if input == submitted && error == "turn admission refused")
    );
    drop(commands_tx);
    task.await
        .expect("relay task joins")
        .expect("session cleanup succeeds");
}

#[tokio::test]
async fn unrouted_roadmap_requests_fail_visibly_without_implementation_submission() {
    let (handle, _events, mut calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let task = spawn_session(Box::new(handle), inbox, commands, None, None, None);
    commands_tx
        .send(Cmd::RoadmapPrompt("Add checks".into()))
        .expect("command sent");
    assert!(matches!(next(&mut messages).await,
        Msg::Agent(AgentEvent::PromptRejected { input, error })
        if input.text == "Add checks" && error.contains("cannot update the roadmap")));
    assert!(calls.try_recv().is_err());
    commands_tx
        .send(Cmd::CancelRoadmapPrompt)
        .expect("cancel sent");
    assert!(matches!(
        next(&mut messages).await,
        Msg::CommandFailed {
            cmd: Cmd::CancelRoadmapPrompt,
            ..
        }
    ));
    assert!(
        calls.try_recv().is_err(),
        "a roadmap cancellation never stops implementation"
    );
    drop(commands_tx);
    task.await.expect("relay joins").expect("cleanup");
}

#[tokio::test]
async fn ending_the_runtime_waits_for_owned_session_cleanup() {
    let (mut handle, _events, mut calls) = fake();
    let (release, cleanup) = oneshot::channel();
    handle.cleanup = Some(cleanup);
    let (inbox, _messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let task = spawn_session(Box::new(handle), inbox, commands, None, None, None);
    drop(commands_tx);
    assert_eq!(next(&mut calls).await, "terminating");
    assert!(
        !task.is_finished(),
        "session cleanup must finish before relay shutdown"
    );
    release.send(()).expect("cleanup released");
    task.await
        .expect("relay task joins")
        .expect("session cleanup succeeds");
    assert_eq!(next(&mut calls).await, "terminated");
}

#[tokio::test]
async fn a_closed_observer_terminates_the_owned_session() {
    let (handle, events, mut calls) = fake();
    let (inbox, messages) = Inbox::channel();
    let (_commands_tx, commands) = Commands::channel();
    let task = spawn_session(Box::new(handle), inbox, commands, None, None, None);
    drop(messages);
    events
        .send(AgentEvent::Text("late event".to_owned()))
        .expect("event sent");
    assert_eq!(next(&mut calls).await, "terminating");
    task.await
        .expect("relay task joins")
        .expect("session cleanup succeeds");
    assert_eq!(next(&mut calls).await, "terminated");
}

#[tokio::test]
async fn cleanup_errors_are_reported_to_the_inbox() {
    let (mut handle, _events, _calls) = fake();
    handle.cleanup_error = Some("child cleanup failed".to_owned());
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let task = spawn_session(Box::new(handle), inbox, commands, None, None, None);
    drop(commands_tx);
    assert_eq!(
        task.await.expect("relay task joins"),
        Err("child cleanup failed".to_owned())
    );
    assert!(
        matches!(next(&mut messages).await, Msg::Agent(AgentEvent::Error(error)) if error == "child cleanup failed")
    );
}

#[tokio::test]
async fn provider_observation_ending_preserves_local_effects() {
    let (handle, events, mut calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let workspace = FakeWorkspace::new().with_file("src/main.rs", "fn main() {}");
    let task = spawn_session(
        Box::new(handle),
        inbox,
        commands,
        Some(Box::new(workspace)),
        None,
        None,
    );
    drop(events);
    commands_tx
        .send(Cmd::Workspace(WorkspaceCmd::Open {
            path: "src/main.rs".to_owned(),
        }))
        .expect("command sent");
    assert!(
        matches!(next(&mut messages).await, Msg::Workspace(WorkspaceEvent::Opened { path, content }) if path == "src/main.rs" && content == "fn main() {}")
    );
    assert!(
        calls.try_recv().is_err(),
        "the local read never reaches the session owner"
    );
    drop(commands_tx);
    task.await
        .expect("relay task joins")
        .expect("session cleanup succeeds");
}

/// A workspace whose search blocks until released: the slow port the
/// relay must never wait on.
struct BlockingWorkspace {
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
    searches: Arc<std::sync::atomic::AtomicUsize>,
}

impl WorkspacePort for BlockingWorkspace {
    fn read(&mut self, path: &str) -> Result<String, String> {
        Ok(format!("read {path}"))
    }
    fn undo(&mut self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    fn journal(&mut self) -> Result<Vec<magistr_cockpit::workspace::JournalEntry>, String> {
        Ok(Vec::new())
    }
    fn search(&mut self, query: &str) -> Result<Vec<String>, String> {
        self.searches
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.release
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .recv()
            .map_err(|_| "the gate went away".to_owned())?;
        Ok(vec![format!("hit for {query}")])
    }
    fn map(&mut self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn run(&mut self, _command: &str) -> Result<Vec<magistr_cockpit::workspace::RunRow>, String> {
        Ok(Vec::new())
    }
}

fn blocking() -> (
    BlockingWorkspace,
    std::sync::mpsc::Sender<()>,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let (release, gate) = std::sync::mpsc::channel();
    let searches = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    (
        BlockingWorkspace {
            release: std::sync::Mutex::new(gate),
            searches: Arc::clone(&searches),
        },
        release,
        searches,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slow_workspace_call_never_blocks_provider_observation_or_cancellation() {
    let (handle, events, mut calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let (workspace, release, searches) = blocking();
    let task = spawn_session(
        Box::new(handle),
        inbox,
        commands,
        Some(Box::new(workspace)),
        None,
        None,
    );
    commands_tx
        .send(Cmd::Workspace(WorkspaceCmd::Search {
            query: "slow".to_owned(),
        }))
        .expect("command sent");
    // The search is running on its own thread, held at the gate …
    tokio::time::timeout(Duration::from_secs(2), async {
        while searches.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the search started");
    // … while the provider's events and a cancel still flow through the relay.
    events
        .send(AgentEvent::Text("still streaming".to_owned()))
        .expect("event sent");
    commands_tx.send(Cmd::Cancel).expect("command sent");
    assert!(matches!(
        next(&mut messages).await,
        Msg::Agent(AgentEvent::Text(text)) if text == "still streaming"
    ));
    assert_eq!(next(&mut calls).await, "cancel");
    release.send(()).expect("the gate opens");
    assert!(matches!(
        next(&mut messages).await,
        Msg::Workspace(WorkspaceEvent::Found { query, hits }) if query == "slow" && hits == ["hit for slow"]
    ));
    drop(commands_tx);
    task.await
        .expect("relay task joins")
        .expect("session cleanup succeeds");
}

#[tokio::test]
async fn an_absent_port_is_refused_by_name_rather_than_answered_with_silence() {
    let (handle, _events, _calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let task = spawn_session(Box::new(handle), inbox, commands, None, None, None);
    commands_tx
        .send(Cmd::Workspace(WorkspaceCmd::Map))
        .expect("command sent");
    commands_tx
        .send(Cmd::Engine(magistr_cockpit::cockpit::EngineCmd::Checkpoint))
        .expect("command sent");
    commands_tx
        .send(Cmd::Checkpoint(
            magistr_cockpit::cockpit::CheckpointCmd::Restore {
                id: "cp-2".to_owned(),
            },
        ))
        .expect("command sent");
    assert!(matches!(
        next(&mut messages).await,
        Msg::Workspace(WorkspaceEvent::Failed { cmd, error }) if cmd == magistr_cockpit::cockpit::WorkspaceCmd::Map && error.contains("no workspace")
    ));
    assert!(matches!(
        next(&mut messages).await,
        Msg::Engine(magistr_cockpit::engine::EngineEvent::PortFailed { what, error, .. }) if what == "checkpoint" && error.contains("no engine port")
    ));
    assert!(matches!(
        next(&mut messages).await,
        Msg::Checkpoint(magistr_cockpit::cockpit::CheckpointEvent::Failed { id: Some(id), what, error })
            if id == "cp-2" && what == "restore cp-2" && error.contains("no checkpoint ledger")
    ));
    drop(commands_tx);
    task.await
        .expect("relay task joins")
        .expect("session cleanup succeeds");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn jobs_past_the_capacity_are_refused_and_a_late_answer_is_fenced() {
    let (handle, _events, mut calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let (workspace, release, searches) = blocking();
    let task = spawn_session(
        Box::new(handle),
        inbox,
        commands,
        Some(Box::new(workspace)),
        None,
        None,
    );
    for index in 0..=SessionJobs::CAPACITY {
        commands_tx
            .send(Cmd::Workspace(WorkspaceCmd::Search {
                query: format!("job {index}"),
            }))
            .expect("command sent");
    }
    // Only the first job holds the port; the others queue on its mutex
    // and count as running. The one past the capacity is refused now.
    assert!(matches!(
        next(&mut messages).await,
        Msg::Workspace(WorkspaceEvent::Failed { cmd, error })
            if magistr_cockpit::workspace::describe(&cmd) == format!("search job {}", SessionJobs::CAPACITY)
                && error.contains("already running")
    ));
    tokio::time::timeout(Duration::from_secs(2), async {
        while searches.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the first search started");
    // The runtime ends while the jobs are still held at the gate: the
    // relay fences them, waits its grace, then terminates the session.
    drop(commands_tx);
    let ended = tokio::time::timeout(SessionJobs::JOIN_GRACE * 2, task)
        .await
        .expect("the relay ends within its grace")
        .expect("relay task joins");
    assert!(ended.is_ok(), "{ended:?}");
    // The owner was asked to end only after the jobs were fenced and waited for.
    assert_eq!(next(&mut calls).await, "terminating");
    assert_eq!(next(&mut calls).await, "terminated");
    assert!(matches!(
        next(&mut messages).await,
        Msg::Agent(AgentEvent::Error(error)) if error.contains("still running when the session ended")
    ));
    // The gate opens after the session ended: every held search now
    // answers on its thread, and none of the answers reaches the inbox.
    for _ in 0..SessionJobs::CAPACITY {
        let _ = release.send(());
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        messages.try_recv().is_err(),
        "a late answer never posts on behalf of an ended session"
    );
}

#[tokio::test]
async fn aborting_the_relay_discards_a_late_blocking_result() {
    let (handle, _events, _calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let (workspace, release, searches) = blocking();
    let task = spawn_session(
        Box::new(handle),
        inbox,
        commands,
        Some(Box::new(workspace)),
        None,
        None,
    );
    commands_tx
        .send(Cmd::Workspace(WorkspaceCmd::Search {
            query: "held".to_owned(),
        }))
        .expect("command sent");
    tokio::time::timeout(Duration::from_secs(2), async {
        while searches.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the search started");

    task.abort();
    assert!(task.await.expect_err("relay was aborted").is_cancelled());
    release.send(()).expect("release the running search");
    // The worker holds the last inbox clone. Observing channel closure waits
    // for its completion, so this proves absence without a timing-based sleep.
    let late = tokio::time::timeout(Duration::from_secs(2), messages.recv())
        .await
        .expect("the blocking job released its inbox");
    assert!(late.is_none(), "ended relay published {late:?}");
}

#[test]
fn the_relay_stamps_elapsed_time_without_changing_other_tool_fields() {
    let (handle, _events, _calls) = fake();
    let (inbox, _messages) = Inbox::channel();
    let mut session = Session {
        handle: Box::new(handle),
        inbox,
        workspace: None,
        engine: None,
        checkpoints: None,
        jobs: SessionJobs::new(),
        tool_started: HashMap::from([(
            "tool-1".to_owned(),
            Instant::now() - Duration::from_secs(3),
        )]),
        started: Instant::now() - Duration::from_secs(9),
        input_origin: InputOrigin::ProviderHistory,
    };
    let mut event = AgentEvent::ToolCallUpdate {
        id: "tool-1".to_owned(),
        title: Some("done".to_owned()),
        status: Some(ToolStatus::Completed),
        diff: None,
        duration_ms: None,
        images: Vec::new(),
        record: Box::new(ToolRecord::default()),
    };
    session.stamp(&mut event);
    assert!(
        matches!(event, AgentEvent::ToolCallUpdate { duration_ms: Some(ms), title: Some(title), .. } if ms >= 3000 && title == "done")
    );
    assert!(
        session.tool_started.is_empty(),
        "a terminal tool update releases its clock"
    );
    let mut usage = AgentEvent::Usage {
        context: Some(magistr_application::sessions::ContextUsage {
            used: 12,
            size: 100,
        }),
        cost_cents: Some(4),
        elapsed_secs: None,
    };
    session.stamp(&mut usage);
    assert!(
        matches!(usage, AgentEvent::Usage { elapsed_secs: Some(secs), context: Some(magistr_application::sessions::ContextUsage { used: 12, size: 100 }), cost_cents: Some(4) } if secs >= 9)
    );
}
