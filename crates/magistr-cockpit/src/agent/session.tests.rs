use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use super::*;
use crate::agent::{ToolRecord, TurnEnd};
use crate::cockpit::{WorkbenchCmd, WorkbenchEvent};
use crate::harness::FakeWorkbench;

struct FakeSession {
    events: mpsc::UnboundedReceiver<AgentEvent>,
    calls: mpsc::UnboundedSender<String>,
    cleanup: Option<oneshot::Receiver<()>>,
    cleanup_error: Option<String>,
}

impl SessionHandle for FakeSession {
    type Event = AgentEvent;

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
        matches!(next(&mut messages).await, Msg::Agent(AgentEvent::Error(error)) if error == "turn admission refused")
    );
    drop(commands_tx);
    task.await
        .expect("relay task joins")
        .expect("session cleanup succeeds");
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
