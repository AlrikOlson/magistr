use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use super::*;
use crate::agent::SessionJobs;
use crate::cockpit::{WorkbenchCmd, WorkbenchEvent};
use crate::harness::FakeWorkbench;
use magistr_application::sessions::{ToolRecord, TurnEnd};

struct FakeSession {
    events: mpsc::UnboundedReceiver<AgentEvent>,
    calls: mpsc::UnboundedSender<String>,
    cleanup: Option<oneshot::Receiver<()>>,
    cleanup_error: Option<String>,
}

impl SessionHandle for FakeSession {
    fn submit_turn(&mut self, text: &str) -> Result<(), String> {
        self.calls
            .send(format!("prompt {text}"))
            .expect("call observer");
        if text == "refused" {
            Err("turn admission refused".to_owned())
        } else {
            Ok(())
        }
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
async fn forwards_session_events_without_echoing_prompts_or_owning_permission_ids() {
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
    commands_tx
        .send(Cmd::Prompt("follow-up".to_owned()))
        .expect("command sent");
    assert_eq!(next(&mut calls).await, "prompt follow-up");
    assert!(
        messages.try_recv().is_err(),
        "the model already echoed this prompt"
    );
    drop(commands_tx);
    task.await
        .expect("relay task joins")
        .expect("session cleanup succeeds");
}

#[tokio::test]
async fn routes_commands_to_the_owner_and_reports_admission_errors() {
    let (handle, _events, mut calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let task = spawn_session(Box::new(handle), inbox, commands, None, None, None);
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
        Cmd::Prompt("refused".to_owned()),
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
        matches!(next(&mut messages).await, Msg::Agent(AgentEvent::PromptRejected { text, error }) if text == "refused" && error == "turn admission refused")
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
        .send(Cmd::RoadmapPrompt("Add checks".to_owned()))
        .expect("command sent");
    assert!(matches!(next(&mut messages).await,
        Msg::Agent(AgentEvent::PromptRejected { text, error })
        if text == "Add checks" && error.contains("cannot update the roadmap")));
    assert!(calls.try_recv().is_err());
    commands_tx
        .send(Cmd::CancelRoadmapPrompt)
        .expect("cancel sent");
    assert_eq!(next(&mut messages).await, Msg::RoadmapRequestCancelled);
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
    let workbench = FakeWorkbench::new().with_file("src/main.rs", "fn main() {}");
    let task = spawn_session(
        Box::new(handle),
        inbox,
        commands,
        Some(Box::new(workbench)),
        None,
        None,
    );
    drop(events);
    commands_tx
        .send(Cmd::Workbench(WorkbenchCmd::Open {
            path: "src/main.rs".to_owned(),
        }))
        .expect("command sent");
    assert!(
        matches!(next(&mut messages).await, Msg::Workbench(WorkbenchEvent::Opened { path, content }) if path == "src/main.rs" && content == "fn main() {}")
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

/// A workbench whose search blocks until released: the slow port the
/// relay must never wait on.
struct BlockingWorkbench {
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
    searches: Arc<std::sync::atomic::AtomicUsize>,
}

impl WorkbenchPort for BlockingWorkbench {
    fn read(&mut self, path: &str) -> Result<String, String> {
        Ok(format!("read {path}"))
    }
    fn undo(&mut self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    fn journal(&mut self) -> Result<Vec<crate::workbench::JournalEntry>, String> {
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
    fn run(&mut self, _command: &str) -> Result<Vec<crate::workbench::RunRow>, String> {
        Ok(Vec::new())
    }
}

fn blocking() -> (
    BlockingWorkbench,
    std::sync::mpsc::Sender<()>,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let (release, gate) = std::sync::mpsc::channel();
    let searches = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    (
        BlockingWorkbench {
            release: std::sync::Mutex::new(gate),
            searches: Arc::clone(&searches),
        },
        release,
        searches,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slow_workbench_call_never_blocks_provider_observation_or_cancellation() {
    let (handle, events, mut calls) = fake();
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let (workbench, release, searches) = blocking();
    let task = spawn_session(
        Box::new(handle),
        inbox,
        commands,
        Some(Box::new(workbench)),
        None,
        None,
    );
    commands_tx
        .send(Cmd::Workbench(WorkbenchCmd::Search {
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
        Msg::Workbench(WorkbenchEvent::Found { query, hits }) if query == "slow" && hits == ["hit for slow"]
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
        .send(Cmd::Workbench(WorkbenchCmd::Map))
        .expect("command sent");
    commands_tx
        .send(Cmd::Engine(crate::cockpit::EngineCmd::Checkpoint))
        .expect("command sent");
    commands_tx
        .send(Cmd::Checkpoint(crate::cockpit::CheckpointCmd::Restore {
            id: "cp-2".to_owned(),
        }))
        .expect("command sent");
    assert!(matches!(
        next(&mut messages).await,
        Msg::Workbench(WorkbenchEvent::Failed { cmd, error }) if cmd == crate::cockpit::WorkbenchCmd::Map && error.contains("no workbench")
    ));
    assert!(matches!(
        next(&mut messages).await,
        Msg::Engine(crate::engine::EngineEvent::PortFailed { what, error, .. }) if what == "checkpoint" && error.contains("no engine port")
    ));
    assert!(matches!(
        next(&mut messages).await,
        Msg::Checkpoint(crate::cockpit::CheckpointEvent::Failed { id: Some(id), what, error })
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
    let (workbench, release, searches) = blocking();
    let task = spawn_session(
        Box::new(handle),
        inbox,
        commands,
        Some(Box::new(workbench)),
        None,
        None,
    );
    for index in 0..=SessionJobs::CAPACITY {
        commands_tx
            .send(Cmd::Workbench(WorkbenchCmd::Search {
                query: format!("job {index}"),
            }))
            .expect("command sent");
    }
    // Only the first job holds the port; the others queue on its mutex
    // and count as running. The one past the capacity is refused now.
    assert!(matches!(
        next(&mut messages).await,
        Msg::Workbench(WorkbenchEvent::Failed { cmd, error })
            if crate::workbench::describe(&cmd) == format!("search job {}", SessionJobs::CAPACITY)
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

#[test]
fn the_relay_stamps_elapsed_time_without_changing_other_tool_fields() {
    let (handle, _events, _calls) = fake();
    let (inbox, _messages) = Inbox::channel();
    let mut session = Session {
        handle: Box::new(handle),
        inbox,
        workbench: None,
        engine: None,
        checkpoints: None,
        jobs: SessionJobs::new(),
        tool_started: HashMap::from([(
            "tool-1".to_owned(),
            Instant::now() - Duration::from_secs(3),
        )]),
        started: Instant::now() - Duration::from_secs(9),
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
        used: 12,
        size: 100,
        cost_cents: Some(4),
        elapsed_secs: None,
    };
    session.stamp(&mut usage);
    assert!(
        matches!(usage, AgentEvent::Usage { elapsed_secs: Some(secs), used: 12, size: 100, cost_cents: Some(4) } if secs >= 9)
    );
}
