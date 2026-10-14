use super::*;

#[path = "interactive_session_context.tests.rs"]
mod context;

#[tokio::test]
async fn explicit_reconnect_restores_the_same_identity_without_sending_a_prompt() {
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (opened, mut connections) = mpsc::unbounded_channel();
    let (handle, opening) = controlled(SessionPurpose::Conversation);
    let task = InteractiveSession::new(
        move |purpose, resume| {
            let (handle, observed) = controlled(purpose);
            opened.send((resume, observed)).unwrap();
            async { Ok(handle) }
        },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, false)
    .spawn(receiver);
    opening
        .events
        .send(AgentEvent::Initialized {
            session_id: "same-provider".into(),
            model: None,
        })
        .unwrap();
    next_observation(&mut events).await;
    opening
        .events
        .send(AgentEvent::Activity(
            magistr_application::sessions::SessionActivity {
                active: false,
                queued_turns: 0,
                configuration_in_progress: false,
                cancellation_requested: false,
                can_resume: Some(true),
                can_send_images: Some(false),
            },
        ))
        .unwrap();
    next_observation(&mut events).await;
    opening
        .events
        .send(AgentEvent::Error("lost".into()))
        .unwrap();
    next_observation(&mut events).await;
    commands.send(Cmd::Reconnect).unwrap();
    let (resume, mut restored) = next(&mut connections).await;
    assert_eq!(resume.as_deref(), Some("same-provider"));
    assert!(
        restored.prompts.try_recv().is_err(),
        "reconnect never manufactures a prompt"
    );
    restored
        .events
        .send(AgentEvent::Text("replayed".into()))
        .unwrap();
    restored
        .events
        .send(AgentEvent::Initialized {
            session_id: "same-provider".into(),
            model: None,
        })
        .unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::Agent(AgentEvent::Initialized { .. })
    ));
    commands.send(Cmd::Quit).unwrap();
    task.await.unwrap().unwrap();
}

use magistr_application::sessions::TurnEnd;
use tokio::sync::mpsc;

struct ControlledSession {
    conversation: Conversation,
    events: mpsc::UnboundedReceiver<AgentEvent>,
    prompts: mpsc::UnboundedSender<String>,
    calls: mpsc::UnboundedSender<Cmd>,
}
impl SessionHandle for ControlledSession {
    fn conversation(&self) -> Option<Conversation> {
        Some(self.conversation.clone())
    }
    fn submit_turn(
        &mut self,
        input: &magistr_application::sessions::PromptInput,
    ) -> Result<(), String> {
        self.prompts
            .send(input.text.clone())
            .map_err(|e| e.to_string())
    }
    fn rename(&mut self, _: &str) -> Result<(), String> {
        Err("Naming is unavailable in this session.".into())
    }
    fn configure(&mut self, _: &str, _: &str) -> Result<(), String> {
        Err("Agent choices are unavailable in this session.".into())
    }
    fn cancel_turn(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn answer_permission(&mut self, id: u64, option_id: &str) -> Result<(), String> {
        self.calls
            .send(Cmd::Permission {
                id,
                option_id: option_id.into(),
            })
            .map_err(|error| error.to_string())
    }
    fn constrain_permission(&mut self, _: u64, _: &str) -> Result<(), String> {
        unreachable!()
    }
    fn delegate_permission(&mut self, _: &str) -> Result<(), String> {
        unreachable!()
    }
    fn kill_terminal(&mut self, _: &str) -> Result<(), String> {
        unreachable!()
    }
    fn detach_client(&mut self) {}
    fn next_event(&mut self) -> Pin<Box<dyn Future<Output = Option<AgentEvent>> + Send + '_>> {
        Box::pin(self.events.recv())
    }
    fn terminate_session(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}
struct Opened {
    purpose: SessionPurpose,
    events: mpsc::UnboundedSender<AgentEvent>,
    prompts: mpsc::UnboundedReceiver<String>,
    calls: mpsc::UnboundedReceiver<Cmd>,
}
fn controlled(purpose: SessionPurpose) -> (Box<dyn SessionHandle>, Opened) {
    let (events, receiver) = mpsc::unbounded_channel();
    let (prompts, requests) = mpsc::unbounded_channel();
    let (calls, observations) = mpsc::unbounded_channel();
    (
        Box::new(ControlledSession {
            conversation: Conversation {
                id: match purpose {
                    SessionPurpose::Conversation => "logical-conversation",
                    SessionPurpose::Roadmap => "logical-roadmap",
                }
                .into(),
                name: None,
                provider: "controlled".into(),
                purpose,
            },
            events: receiver,
            prompts,
            calls,
        }),
        Opened {
            purpose,
            events,
            prompts: requests,
            calls: observations,
        },
    )
}
async fn next<T>(receiver: &mut mpsc::UnboundedReceiver<T>) -> T {
    tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .expect("progress")
        .expect("open")
}

#[tokio::test]
async fn busy_conversation_followups_reach_the_same_owner_and_are_accepted() {
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (handle, mut opening) = controlled(SessionPurpose::Conversation);
    let task = InteractiveSession::new(
        |_, _| async { Err("must keep the resident connection".into()) },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, true)
    .spawn(receiver);
    commands
        .send(Cmd::Prompt("accepted follow-up".into()))
        .unwrap();
    assert_eq!(next(&mut opening.prompts).await, "accepted follow-up");
    assert!(
        matches!(next(&mut events).await, Msg::InputAccepted(text) if text.text.trim_end() == "accepted follow-up")
    );
    commands.send(Cmd::Quit).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn reconnect_restores_each_purpose_and_omits_replayed_history() {
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (opened, mut connections) = mpsc::unbounded_channel();
    let (handle, mut opening) = controlled(SessionPurpose::Conversation);
    let task = InteractiveSession::new(
        move |purpose, resume| {
            let (handle, observed) = controlled(purpose);
            opened.send((resume, observed)).unwrap();
            async { Ok(handle) }
        },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, false)
    .spawn(receiver);
    // Initial explicit resume must still show its history.
    opening
        .events
        .send(AgentEvent::Text("Opening history".into()))
        .unwrap();
    assert!(
        matches!(next_observation(&mut events).await, Msg::Agent(AgentEvent::Text(text)) if text == "Opening history")
    );
    opening
        .events
        .send(AgentEvent::Initialized {
            session_id: "conversation".into(),
            model: None,
        })
        .unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::Agent(AgentEvent::Initialized { .. })
    ));
    // Park the conversation while planning acquires its own identity.
    commands
        .send(Cmd::RoadmapPrompt("First plan".into()))
        .unwrap();
    let (resume, mut planning) = next(&mut connections).await;
    assert_eq!(resume, None);
    assert_eq!(planning.purpose, SessionPurpose::Roadmap);
    assert_eq!(next(&mut planning.prompts).await, "First plan");
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::RoadmapRequestStarted
    ));
    planning
        .events
        .send(AgentEvent::Initialized {
            session_id: "planning".into(),
            model: None,
        })
        .unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::Agent(AgentEvent::Initialized { .. })
    ));
    planning
        .events
        .send(AgentEvent::TurnEnded(TurnEnd::EndTurn))
        .unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::Agent(AgentEvent::TurnEnded(_))
    ));
    commands
        .send(Cmd::Prompt("Continue conversation".into()))
        .unwrap();
    assert_eq!(next(&mut opening.prompts).await, "Continue conversation");
    let mut restored_sessions = Vec::new();
    for (current, command, identity, purpose) in [
        (
            opening,
            Cmd::Prompt("Retry conversation".into()),
            "conversation",
            SessionPurpose::Conversation,
        ),
        (
            planning,
            Cmd::RoadmapPrompt("Retry planning".into()),
            "planning",
            SessionPurpose::Roadmap,
        ),
    ] {
        // The parked provider may fail before it is selected again.
        current
            .events
            .send(AgentEvent::Error("provider died".into()))
            .unwrap();
        assert!(matches!(
            next_observation(&mut events).await,
            Msg::Agent(AgentEvent::Error(_))
        ));
        commands.send(command).unwrap();
        let (resume, mut restored) = next(&mut connections).await;
        assert_eq!(resume.as_deref(), Some(identity));
        assert_eq!(restored.purpose, purpose);
        assert!(next(&mut restored.prompts).await.starts_with("Retry"));
        if purpose == SessionPurpose::Roadmap {
            assert!(matches!(
                next_observation(&mut events).await,
                Msg::RoadmapRequestStarted
            ));
        }
        restored
            .events
            .send(AgentEvent::UserText("Old request".into()))
            .unwrap();
        restored
            .events
            .send(AgentEvent::Text("Old response".into()))
            .unwrap();
        restored
            .events
            .send(AgentEvent::Initialized {
                session_id: identity.into(),
                model: None,
            })
            .unwrap();
        // The first displayed event must be the new connection, not replay.
        assert!(matches!(
            next_observation(&mut events).await,
            Msg::Agent(AgentEvent::Initialized { .. })
        ));
        restored
            .events
            .send(AgentEvent::Text("New response".into()))
            .unwrap();
        assert!(
            matches!(next_observation(&mut events).await, Msg::Agent(AgentEvent::Text(text)) if text == "New response")
        );
        restored
            .events
            .send(AgentEvent::TurnEnded(TurnEnd::EndTurn))
            .unwrap();
        assert!(matches!(
            next_observation(&mut events).await,
            Msg::Agent(AgentEvent::TurnEnded(_))
        ));
        // Keep the connection alive while the other purpose is selected.
        restored_sessions.push(restored);
    }
    drop(commands);
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn failed_restoration_keeps_the_identity_and_rejects_the_unsent_request() {
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (attempted, mut attempts) = mpsc::unbounded_channel();
    let (handle, opening) = controlled(SessionPurpose::Conversation);
    let task = InteractiveSession::new(
        move |purpose, resume| {
            attempted.send((purpose, resume)).unwrap();
            async { Err("This provider cannot restore the session".into()) }
        },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, false)
    .spawn(receiver);
    opening
        .events
        .send(AgentEvent::Initialized {
            session_id: "retained".into(),
            model: None,
        })
        .unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::Agent(AgentEvent::Initialized { .. })
    ));
    opening
        .events
        .send(AgentEvent::Error("provider disconnected".into()))
        .unwrap();
    drop(opening.events);
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::Agent(AgentEvent::Error(_))
    ));
    for prompt in ["Keep this request", "Try again"] {
        commands.send(Cmd::Prompt(prompt.into())).unwrap();
        assert_eq!(
            next(&mut attempts).await,
            (SessionPurpose::Conversation, Some("retained".into()))
        );
        assert!(matches!(next_observation(&mut events).await,
            Msg::Agent(AgentEvent::PromptRejected { input, error })
                if input.text == prompt && error.contains("cannot restore")));
    }
    drop(commands);
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn planning_waits_for_the_turn_and_keeps_a_separate_reusable_context() {
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (opened, mut connections) = mpsc::unbounded_channel();
    let task = InteractiveSession::new(
        move |purpose, _resume| {
            let (handle, observed) = controlled(purpose);
            opened.send(observed).unwrap();
            async { Ok(handle) }
        },
        inbox,
        None,
        None,
        None,
    )
    .spawn(receiver);
    commands.send(Cmd::Prompt("Implement".into())).unwrap();
    let mut conversation = next(&mut connections).await;
    assert_eq!(conversation.purpose, SessionPurpose::Conversation);
    assert_eq!(next(&mut conversation.prompts).await, "Implement");

    let discarded = magistr_application::sessions::PromptInput {
        text: "Discard this request".into(),
        attachments: vec![
            magistr_application::sessions::PromptAttachment::from_bytes(
                "queued.txt",
                "selected-hash",
                b"unsent attachment".to_vec(),
            )
            .unwrap(),
        ],
    };
    commands
        .send(Cmd::RoadmapPrompt(discarded.clone()))
        .unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::RoadmapRequestQueued
    ));
    commands.send(Cmd::CancelRoadmapPrompt).unwrap();
    assert!(matches!(
        next(&mut events).await,
        Msg::Agent(AgentEvent::PromptRejected { input, error })
            if input == discarded && error == "This send was stopped. Your draft is still available."
    ));
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::RoadmapRequestCancelled
    ));
    assert!(connections.try_recv().is_err());
    commands.send(Cmd::RoadmapPrompt("Plan".into())).unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::RoadmapRequestQueued
    ));
    assert!(
        connections.try_recv().is_err(),
        "planning cannot start during the turn"
    );
    conversation
        .events
        .send(AgentEvent::TurnEnded(TurnEnd::EndTurn))
        .unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::Agent(AgentEvent::TurnEnded(_))
    ));
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::RoadmapRequestStarted
    ));
    let mut planning = next(&mut connections).await;
    assert_eq!(planning.purpose, SessionPurpose::Roadmap);
    assert_eq!(next(&mut planning.prompts).await, "Plan");

    planning
        .events
        .send(AgentEvent::Text("Planning reply".into()))
        .unwrap();
    assert!(
        matches!(next_observation(&mut events).await, Msg::Agent(AgentEvent::Text(text)) if text == "Planning reply")
    );
    planning
        .events
        .send(AgentEvent::TurnEnded(TurnEnd::EndTurn))
        .unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::Agent(AgentEvent::TurnEnded(_))
    ));
    commands.send(Cmd::Prompt("Continue".into())).unwrap();
    assert_eq!(next(&mut conversation.prompts).await, "Continue");
    assert!(
        connections.try_recv().is_err(),
        "the conversation keeps its provider context"
    );
    conversation
        .events
        .send(AgentEvent::TurnEnded(TurnEnd::EndTurn))
        .unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::Agent(AgentEvent::TurnEnded(_))
    ));
    commands.send(Cmd::RoadmapPrompt("Refine".into())).unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::RoadmapRequestStarted
    ));
    assert_eq!(next(&mut planning.prompts).await, "Refine");
    assert!(
        connections.try_recv().is_err(),
        "planning keeps its own provider context"
    );
    drop(commands);
    task.await.unwrap().unwrap();
    assert!(conversation.events.is_closed() && planning.events.is_closed());
}

#[tokio::test]
async fn cancelling_a_pending_connection_rejects_exact_input_and_allows_retry() {
    use magistr_application::sessions::{PromptAttachment, PromptInput};
    for (roadmap, abort) in [(false, false), (true, false), (false, true), (true, true)] {
        let (inbox, mut events) = Inbox::channel();
        let (commands, receiver) = Commands::channel();
        let (opened, mut connections) = mpsc::unbounded_channel();
        let task = InteractiveSession::new(
            move |purpose, _resume| {
                let (handle, observed) = controlled(purpose);
                let (release, wait) = tokio::sync::oneshot::channel();
                opened.send((observed, release)).unwrap();
                async move {
                    wait.await
                        .map_err(|_| "The connection was cancelled.".to_owned())?;
                    Ok(handle)
                }
            },
            inbox,
            None,
            None,
            None,
        )
        .spawn(receiver);
        let input = PromptInput {
            text: "Keep this request".into(),
            attachments: vec![
                PromptAttachment::from_bytes(
                    "note.txt",
                    "selected-hash",
                    "exact\r\n😀\n".as_bytes().to_vec(),
                )
                .unwrap(),
            ],
        };
        let prompt = if roadmap {
            Cmd::RoadmapPrompt(input.clone())
        } else {
            Cmd::Prompt(input.clone())
        };
        commands.send(prompt.clone()).unwrap();
        let (mut cancelled, release) = next(&mut connections).await;
        let cancellation = if abort {
            Cmd::Loop(LoopCmd::Abort)
        } else if roadmap {
            Cmd::CancelRoadmapPrompt
        } else {
            Cmd::Cancel
        };
        commands.send(cancellation).unwrap();
        assert!(matches!(
            next(&mut events).await,
            Msg::Agent(AgentEvent::PromptRejected { input: rejected, error })
                if rejected == input && error == "This send was stopped. Your draft is still available."
        ));
        let terminal = next(&mut events).await;
        if roadmap {
            assert!(matches!(terminal, Msg::RoadmapRequestCancelled));
        } else {
            assert!(matches!(
                terminal,
                Msg::Agent(AgentEvent::TurnEnded(TurnEnd::Cancelled))
            ));
        }
        if abort {
            assert!(matches!(
                next(&mut events).await,
                Msg::CommandFailed {
                    cmd: Cmd::Loop(LoopCmd::Abort),
                    ..
                }
            ));
        }
        assert!(
            release.send(()).is_err(),
            "the cancelled connector was dropped"
        );
        assert_eq!(
            cancelled.prompts.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        );
        assert!(
            events.try_recv().is_err(),
            "the input is rejected exactly once"
        );

        commands.send(prompt).unwrap();
        let (mut retry, release) = next(&mut connections).await;
        release.send(()).unwrap();
        assert_eq!(next(&mut retry.prompts).await, input.text);
        if roadmap {
            assert!(matches!(
                next(&mut events).await,
                Msg::RoadmapRequestStarted
            ));
        }
        assert!(
            matches!(next(&mut events).await, Msg::InputAccepted(receipt) if receipt == input.receipt())
        );
        assert!(events.try_recv().is_err(), "only the retry was accepted");
        commands.send(Cmd::Quit).unwrap();
        task.await.unwrap().unwrap();
    }
}

#[tokio::test]
async fn connection_failure_and_provider_death_allow_a_new_attempt() {
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (opened, mut connections) = mpsc::unbounded_channel();
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = Arc::clone(&attempts);
    let task = InteractiveSession::new(
        move |purpose, _resume| {
            let result = if observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                Err("offline".into())
            } else {
                let (handle, observed) = controlled(purpose);
                opened.send(observed).unwrap();
                Ok(handle)
            };
            async { result }
        },
        inbox,
        None,
        None,
        None,
    )
    .spawn(receiver);
    commands
        .send(Cmd::Prompt("Keep this request".into()))
        .unwrap();
    assert!(
        matches!(next_observation(&mut events).await, Msg::Agent(AgentEvent::PromptRejected { input, error }) if input.text == "Keep this request" && error.contains("offline"))
    );
    commands.send(Cmd::Prompt("Retry".into())).unwrap();
    let mut first = next(&mut connections).await;
    assert_eq!(next(&mut first.prompts).await, "Retry");
    first
        .events
        .send(AgentEvent::Error("provider died".into()))
        .unwrap();
    assert!(
        matches!(next_observation(&mut events).await, Msg::Agent(AgentEvent::Error(error)) if error == "provider died")
    );
    commands.send(Cmd::Prompt("Reconnect".into())).unwrap();
    let mut second = next(&mut connections).await;
    assert_eq!(next(&mut second.prompts).await, "Reconnect");
    assert!(
        first.events.is_closed(),
        "the failed connection was released"
    );
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
    drop(commands);
    task.await.unwrap().unwrap();
}

struct HeldWorkbench {
    started: Arc<tokio::sync::Notify>,
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}
impl WorkbenchPort for HeldWorkbench {
    fn read(&mut self, _: &str) -> Result<String, String> {
        self.started.notify_one();
        self.release
            .lock()
            .unwrap()
            .recv()
            .map_err(|e| e.to_string())?;
        Ok("late result".into())
    }
    fn undo(&mut self, _: &str) -> Result<(), String> {
        unreachable!()
    }
    fn journal(&mut self) -> Result<Vec<magistr_cockpit::workbench::JournalEntry>, String> {
        unreachable!()
    }
    fn search(&mut self, _: &str) -> Result<Vec<String>, String> {
        unreachable!()
    }
    fn map(&mut self) -> Result<Vec<String>, String> {
        unreachable!()
    }
    fn run(&mut self, _: &str) -> Result<Vec<magistr_cockpit::workbench::RunRow>, String> {
        unreachable!()
    }
}

#[tokio::test]
async fn aborting_owner_releases_both_contexts_and_fences_a_running_local_job() {
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (opened, mut connections) = mpsc::unbounded_channel();
    let started = Arc::new(tokio::sync::Notify::new());
    let (release, gate) = std::sync::mpsc::channel();
    let task = InteractiveSession::new(
        move |purpose, _resume| {
            let (handle, observed) = controlled(purpose);
            opened.send(observed).unwrap();
            async { Ok(handle) }
        },
        inbox,
        Some(Box::new(HeldWorkbench {
            started: Arc::clone(&started),
            release: std::sync::Mutex::new(gate),
        })),
        None,
        None,
    )
    .spawn(receiver);
    commands.send(Cmd::Prompt("Conversation".into())).unwrap();
    let mut conversation = next(&mut connections).await;
    next(&mut conversation.prompts).await;
    conversation
        .events
        .send(AgentEvent::TurnEnded(TurnEnd::EndTurn))
        .unwrap();
    next_observation(&mut events).await;
    commands
        .send(Cmd::RoadmapPrompt("Planning".into()))
        .unwrap();
    let mut planning = next(&mut connections).await;
    next(&mut planning.prompts).await;
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::RoadmapRequestStarted
    ));
    assert!(
        matches!(next(&mut events).await, Msg::InputAccepted(input) if input.text == "Planning")
    );
    commands
        .send(Cmd::Workbench(WorkbenchCmd::Open {
            path: "held.rs".into(),
        }))
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), async {
        conversation.events.closed().await;
        planning.events.closed().await;
    })
    .await
    .expect("both provider relays released");
    // Let cancellation run on the local relay before releasing its blocking job.
    tokio::task::yield_now().await;
    release.send(()).unwrap();
    let late = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap();
    assert!(late.is_none(), "cancelled owner published {late:?}");
}

struct ObservedWorkbench(Arc<std::sync::atomic::AtomicUsize>);

impl WorkbenchPort for ObservedWorkbench {
    fn read(&mut self, _: &str) -> Result<String, String> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok("unchanged".into())
    }
    fn undo(&mut self, _: &str) -> Result<(), String> {
        unreachable!()
    }
    fn journal(&mut self) -> Result<Vec<magistr_cockpit::workbench::JournalEntry>, String> {
        unreachable!()
    }
    fn search(&mut self, _: &str) -> Result<Vec<String>, String> {
        unreachable!()
    }
    fn map(&mut self) -> Result<Vec<String>, String> {
        unreachable!()
    }
    fn run(&mut self, _: &str) -> Result<Vec<magistr_cockpit::workbench::RunRow>, String> {
        unreachable!()
    }
}

#[tokio::test]
async fn dropping_the_owner_discards_buffered_local_actions() {
    let (inbox, mut messages) = Inbox::channel();
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let session = InteractiveSession::new(
        |_, _| async { panic!("local actions do not open a provider") },
        inbox,
        Some(Box::new(ObservedWorkbench(Arc::clone(&reads)))),
        None,
        None,
    );
    assert!(session.forward(Cmd::Workbench(WorkbenchCmd::Open {
        path: "kept.rs".into()
    })));
    // No yield: the local relay has not consumed the queued operation. Closing
    // its owner must cancel that relay, not detach it to drain abandoned work.
    drop(session);
    let event = tokio::time::timeout(Duration::from_secs(2), messages.recv())
        .await
        .unwrap();
    assert!(
        event.is_none(),
        "closed owner executed a buffered action: {event:?}"
    );
    assert_eq!(
        reads.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "closed owner ran abandoned work"
    );
}

use magistr_cockpit::cockpit::{EngineCmd, WorkbenchCmd};

#[tokio::test]
async fn closed_local_and_provider_receivers_preserve_the_refused_action() {
    let (inbox, mut events) = Inbox::channel();
    let mut session = InteractiveSession::new(
        |_, _| async { panic!("forwarding must not start a provider") },
        inbox,
        None,
        None,
        None,
    );
    session.local_task.abort();
    let _ = (&mut session.local_task).await;
    let (provider, receiver) = Commands::channel();
    drop(receiver);
    let (_, provider_events) = Inbox::channel();
    session.connection = Some(Connection {
        task: tokio::spawn(async { Ok(()) }),
        commands: Some(provider),
        events: provider_events,
        conversation: Conversation {
            id: "logical-conversation".into(),
            name: None,
            provider: "controlled".into(),
            purpose: SessionPurpose::Conversation,
        },
        inbox: session.inbox.for_conversation("logical-conversation"),
        roadmap: false,
        replaying: false,
    });
    for cmd in [
        Cmd::Prompt("Keep @src/main.rs".into()),
        Cmd::Cancel,
        Cmd::Permission {
            id: 5,
            option_id: "allow".into(),
        },
        Cmd::Workbench(WorkbenchCmd::Undo { id: "op-2".into() }),
        Cmd::Engine(EngineCmd::Checkpoint),
    ] {
        assert!(!session.forward(cmd.clone()));
        assert!(matches!(events.recv().await,
            Some(Msg::CommandFailed { cmd: refused, error }) if refused == cmd && error.contains("connection closed")));
        assert!(
            events.try_recv().is_err(),
            "one outcome for one failed action"
        );
    }
    session.close().await;
}

#[tokio::test]
async fn delivery_to_an_open_receiver_does_not_emit_a_refusal() {
    let (inbox, mut events) = Inbox::channel();
    let mut session = InteractiveSession::new(
        |_, _| async { panic!("forwarding must not start a provider") },
        inbox,
        None,
        None,
        None,
    );
    let (provider, mut receiver) = Commands::channel();
    let (_, provider_events) = Inbox::channel();
    session.connection = Some(Connection {
        task: tokio::spawn(async { Ok(()) }),
        commands: Some(provider),
        events: provider_events,
        conversation: Conversation {
            id: "logical-conversation".into(),
            name: None,
            provider: "controlled".into(),
            purpose: SessionPurpose::Conversation,
        },
        inbox: session.inbox.for_conversation("logical-conversation"),
        roadmap: false,
        replaying: false,
    });
    assert!(session.forward(Cmd::Cancel));
    assert_eq!(receiver.next().await, Some(Cmd::Cancel));
    assert!(events.try_recv().is_err());
    session.close().await;
    session.local_task.abort();
    let _ = (&mut session.local_task).await;
}

#[tokio::test]
async fn unsupported_controls_and_absent_provider_fail_without_ending_live_work() {
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let task = spawn_session(
        Box::new(crate::local_session::LocalSession),
        inbox,
        commands,
        None,
        None,
        None,
    );
    for cmd in [
        Cmd::Cancel,
        Cmd::Permission {
            id: 17,
            option_id: "allow".into(),
        },
        Cmd::Constraint {
            id: 17,
            condition: "read only".into(),
        },
        Cmd::Delegate("read".into()),
        Cmd::Kill(magistr_cockpit::cockpit::KillCmd::Terminal("build".into())),
        Cmd::Kill(magistr_cockpit::cockpit::KillCmd::Gate("checks".into())),
        Cmd::Reconnect,
        Cmd::Loop(magistr_cockpit::cockpit::LoopCmd::Pause),
    ] {
        commands_tx.send(cmd.clone()).unwrap();
        let event = tokio::time::timeout(Duration::from_secs(2), messages.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(&event, Msg::CommandFailed { cmd: refused, error } if refused == &cmd && !error.is_empty())
        );
    }
    drop(commands_tx);
    task.await.unwrap().unwrap();
}

fn idle_session(inbox: Inbox) -> (InteractiveSession, Opened) {
    let (handle, opening) = controlled(SessionPurpose::Conversation);
    let session = InteractiveSession::new(
        |purpose, _| {
            let (handle, _) = controlled(purpose);
            async { Ok(handle) }
        },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, false)
    .admitting_work(|action| match action {
        magistr_cockpit::cockpit::LaunchAction::Gates => Err("The checks are not ready.".into()),
        _ => Ok(()),
    });
    (session, opening)
}

#[tokio::test]
async fn an_idle_session_yields_a_start_to_its_host_with_the_commands_still_open() {
    use magistr_cockpit::cockpit::LaunchAction;
    let (inbox, mut events) = Inbox::channel();
    let (commands, mut receiver) = Commands::channel();
    let (session, _opening) = idle_session(inbox);
    commands
        .send(Cmd::Launch(LaunchAction::Loop(
            magistr_cockpit::cockpit::LoopScope::AllTracks,
        )))
        .unwrap();
    commands.send(Cmd::Prompt("after the work".into())).unwrap();
    let exit = tokio::time::timeout(
        Duration::from_secs(2),
        session.run(&mut receiver, None, false),
    )
    .await
    .expect("the start ends the conversation phase");
    assert!(exit.open, "the cockpit is still there");
    assert_eq!(
        exit.launch,
        Some(LaunchAction::Loop(
            magistr_cockpit::cockpit::LoopScope::AllTracks
        ))
    );
    assert!(events.try_recv().is_err(), "a start is not a failure");
    assert_eq!(
        receiver.next().await,
        Some(Cmd::Prompt("after the work".into())),
        "what followed the start is the host's to route"
    );
}

#[tokio::test]
async fn a_start_during_a_response_or_inside_running_work_is_refused_in_words() {
    use magistr_cockpit::cockpit::LaunchAction;
    // A response in flight: the start waits for the reader to ask again.
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (session, mut opening) = idle_session(inbox);
    let task = session.spawn(receiver);
    commands.send(Cmd::Prompt("think".into())).unwrap();
    assert_eq!(next(&mut opening.prompts).await, "think");
    commands.send(Cmd::Launch(LaunchAction::Next)).unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::CommandFailed { cmd: Cmd::Launch(LaunchAction::Next), error }
            if error.contains("current response")
    ));
    drop(commands);
    task.await.unwrap().unwrap();

    // A start the host refuses, or nobody admits, leaves the conversation open.
    let (inbox, mut events) = Inbox::channel();
    let (commands, mut receiver) = Commands::channel();
    let (session, mut opening) = idle_session(inbox);
    commands.send(Cmd::Launch(LaunchAction::Gates)).unwrap();
    commands.send(Cmd::Prompt("still here".into())).unwrap();
    let running = tokio::spawn(async move { session.run(&mut receiver, None, false).await });
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::CommandFailed { cmd: Cmd::Launch(LaunchAction::Gates), error }
            if error == "The checks are not ready."
    ));
    assert_eq!(next(&mut opening.prompts).await, "still here");
    drop(commands);
    assert_eq!(running.await.unwrap().launch, None);

    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (handle, _unadmitted) = controlled(SessionPurpose::Conversation);
    let task = InteractiveSession::new(
        |purpose, _| {
            let (handle, _) = controlled(purpose);
            async { Ok(handle) }
        },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, false)
    .spawn(receiver);
    commands.send(Cmd::Launch(LaunchAction::Next)).unwrap();
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::CommandFailed { cmd: Cmd::Launch(_), error } if error.contains("cannot start work")
    ));
    drop(commands);
    task.await.unwrap().unwrap();

    // A follow-up owned by running work never starts a second run.
    let (inbox, mut events) = Inbox::channel();
    let (commands, mut receiver) = Commands::channel();
    let (session, _opening) = idle_session(inbox);
    commands.send(Cmd::Launch(LaunchAction::Next)).unwrap();
    drop(commands);
    let exit = session.run(&mut receiver, None, true).await;
    assert_eq!(exit.launch, None);
    assert!(matches!(
        next_observation(&mut events).await,
        Msg::CommandFailed { cmd: Cmd::Launch(_), error } if error.contains("already running")
    ));
}
/// These coordinator tests assert routing and control events. Admission
/// observations are asserted independently by the relay's companion suite.
async fn next_observation(receiver: &mut mpsc::UnboundedReceiver<Msg>) -> Msg {
    loop {
        let message = next(receiver).await;
        if !matches!(message, Msg::InputAccepted(_)) {
            return message;
        }
    }
}
