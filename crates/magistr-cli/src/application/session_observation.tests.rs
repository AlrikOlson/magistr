use super::*;

fn bind(owner: &SessionObservation) {
    owner.bind_context_for_test(magistr_application::sessions::Conversation {
        id: "one".into(),
        name: None,
        provider: "scripted".into(),
        purpose: magistr_application::sessions::SessionPurpose::Conversation,
    });
}

fn address() -> SessionAddress {
    SessionAddress {
        project: "project".into(),
        workspace: "/workspace".into(),
        session: "one".into(),
    }
}

#[test]
fn drafts_and_answers_advance_the_same_observation_sequence() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    let before = owner.snapshot().sequence;
    bind(&owner);
    owner.draft_for("one", "draft".into()).unwrap();
    assert!(owner.snapshot().sequence > before);
    owner.send(Msg::Agent(AgentEvent::PermissionRequest {
        detail: String::new(),
        id: 7,
        title: "Read".into(),
        choices: Vec::new(),
        edit: false,
        risk: magistr_domain::policy::RiskTier::Low,
        plan: None,
    }));
    let pending = owner.snapshot().sequence;
    owner.permission_answered_for("one", 7).unwrap();
    let snapshot = owner.snapshot();
    assert!(snapshot.sequence > pending);
    assert_eq!(snapshot.pending_requests().count(), 0);
    owner.send(Msg::CommandFailed {
        cmd: magistr_cockpit::cockpit::Cmd::Permission {
            id: 7,
            option_id: "allow".into(),
        },
        error: "delivery failed".into(),
    });
    assert_eq!(owner.snapshot().pending_requests().count(), 1);
}

#[test]
fn current_session_facts_survive_retained_transcript_rotation() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    owner.send(Msg::Agent(AgentEvent::Initialized {
        session_id: "provider".into(),
        model: Some("known model".into()),
    }));
    owner.send(Msg::InputAccepted(PromptInput::from("hello").receipt()));
    for _ in 0..MAX_MESSAGES + 5 {
        owner.send(Msg::Agent(AgentEvent::Notice("reply".into())));
    }
    let snapshot = owner.snapshot();
    assert!(snapshot.dropped > 0);
    assert_eq!(snapshot.model.as_deref(), Some("known model"));
    assert_eq!(snapshot.status, SessionStatus::Working);
    owner.send(Msg::Agent(AgentEvent::TurnEnded(
        magistr_application::sessions::TurnEnd::EndTurn,
    )));
    assert_eq!(owner.snapshot().status, SessionStatus::Finished);
}

#[tokio::test]
async fn disconnected_readers_do_not_close_the_owner_and_reconnect_gets_a_reseed() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    drop(owner.subscribe());
    for _ in 0..MAX_MESSAGES + 20 {
        assert!(owner.send(Msg::Agent(AgentEvent::Notice("text".into()))));
    }
    let mut reader = owner.subscribe();
    let snapshot = reader.snapshot();
    assert_eq!(snapshot.sequence, (MAX_MESSAGES + 20) as u64);
    assert_eq!(snapshot.messages.len(), MAX_MESSAGES);
    assert_eq!(snapshot.dropped, 20);
    assert!(snapshot.since(0).is_none());
    assert!(matches!(
        snapshot.replay().first(),
        Some(Msg::Agent(AgentEvent::Lagged { items: 20, .. }))
    ));
    owner.stop();
    assert!(reader.changed().await.unwrap().closed);
    assert!(!owner.send(Msg::Agent(AgentEvent::Text("late".into()))));
}

#[test]
fn bounded_bytes_and_pending_permission_survive_transcript_rotation() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    bind(&owner);
    let request = Msg::Agent(AgentEvent::PermissionRequest {
        detail: String::new(),
        id: 7,
        title: "Read a file".into(),
        choices: Vec::new(),
        edit: false,
        risk: magistr_domain::policy::RiskTier::Low,
        plan: None,
    });
    owner.send(request);
    owner.send(Msg::Agent(AgentEvent::Text("x".repeat(MAX_BYTES))));
    let snapshot = owner.snapshot();
    assert!(snapshot.bytes <= MAX_BYTES);
    assert!(
        snapshot.since(0).is_none(),
        "an oversized delta remains an explicit gap when retention is empty"
    );
    assert!(snapshot.pending.contains_key(&7));
    assert!(
        snapshot
            .replay()
            .iter()
            .any(|msg| matches!(msg, Msg::Agent(AgentEvent::PermissionRequest { id: 7, .. })))
    );
    owner.permission_answered_for("one", 7).unwrap();
    assert!(owner.is_answered("one", 7));
    owner.send(Msg::Agent(AgentEvent::PermissionCancelled { id: 7 }));
    assert!(!owner.snapshot().pending.contains_key(&7));
}
#[test]
fn usage_survives_rotation_and_too_many_requests_have_a_bounded_explicit_outcome() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    owner.send(Msg::Agent(AgentEvent::Usage {
        context: Some(magistr_application::sessions::ContextUsage {
            used: 100,
            size: 1000,
        }),
        cost_cents: Some(42),
        elapsed_secs: Some(10),
    }));
    for _ in 0..MAX_MESSAGES + 1 {
        owner.send(Msg::Agent(AgentEvent::Notice("stream".into())));
    }
    assert!(owner.snapshot().dropped > 0);
    assert!(owner.snapshot().replay().iter().any(|msg| matches!(
        msg,
        Msg::Agent(AgentEvent::Usage {
            cost_cents: Some(42),
            ..
        })
    )));
    for id in 0..MAX_REQUESTS as u64 {
        assert!(owner.send(Msg::Agent(AgentEvent::PermissionRequest {
            detail: String::new(),
            id,
            title: "Read".into(),
            choices: Vec::new(),
            edit: false,
            risk: magistr_domain::policy::RiskTier::Low,
            plan: None
        })));
    }
    assert!(!owner.send(Msg::Agent(AgentEvent::PermissionRequest {
        detail: String::new(),
        id: MAX_REQUESTS as u64,
        title: "Too many".into(),
        choices: Vec::new(),
        edit: false,
        risk: magistr_domain::policy::RiskTier::Low,
        plan: None
    })));
    let snapshot = owner.snapshot();
    assert!(snapshot.closed);
    assert_eq!(snapshot.pending.len(), MAX_REQUESTS);
    assert!(snapshot.replay().iter().any(|msg| matches!(msg, Msg::Agent(AgentEvent::Error(error)) if error.contains("too many requests"))));
}
#[test]
fn rejected_messages_remain_drafts_after_rotation_without_replacing_newer_input() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    owner.send(Msg::Agent(AgentEvent::PromptRejected {
        input: "unsent".into(),
        error: "provider unavailable".into(),
    }));
    for _ in 0..MAX_MESSAGES + 1 {
        owner.send(Msg::Agent(AgentEvent::Notice("other stream".into())));
    }
    assert!(owner.snapshot().dropped > 0);
    assert_eq!(owner.snapshot().seed.draft.as_deref(), Some("unsent"));
    bind(&owner);
    owner.draft_for("one", "newer input".into()).unwrap();
    owner.send(Msg::Agent(AgentEvent::PromptRejected {
        input: "old".into(),
        error: "closed".into(),
    }));
    assert_eq!(owner.snapshot().seed.draft.as_deref(), Some("newer input"));
}

#[tokio::test]
async fn checking_closure_does_not_consume_unread_observations() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    let mut reader = owner.subscribe();
    owner.send(Msg::Agent(AgentEvent::Text("unread".into())));
    assert!(!reader.is_closed());
    let snapshot = tokio::time::timeout(std::time::Duration::from_millis(100), reader.changed())
        .await
        .expect("status inspection must not consume the update")
        .unwrap();
    assert_eq!(snapshot.sequence, 1);
    owner.stop();
    assert!(reader.is_closed());
}

#[test]
fn finished_and_closed_sessions_keep_reported_spending_for_reconnected_readers() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    owner.send(Msg::Agent(AgentEvent::Usage {
        context: Some(magistr_application::sessions::ContextUsage { used: 8, size: 100 }),
        cost_cents: Some(25),
        elapsed_secs: None,
    }));
    owner.send(Msg::Agent(AgentEvent::TurnEnded(
        magistr_application::sessions::TurnEnd::EndTurn,
    )));
    assert!(matches!(
        owner.snapshot().latest_usage(),
        Some(Msg::Agent(AgentEvent::Usage {
            cost_cents: Some(25),
            ..
        }))
    ));
    owner.stop();
    let mut reader = owner.subscribe();
    let snapshot = reader.snapshot();
    assert!(snapshot.closed);
    assert!(matches!(
        snapshot.latest_usage(),
        Some(Msg::Agent(AgentEvent::Usage {
            cost_cents: Some(25),
            ..
        }))
    ));
}
#[test]
fn late_tool_updates_replace_the_original_record_in_owner_order_and_replay() {
    use magistr_application::sessions::{ToolPayload, ToolRecord, ToolStatus, ToolVerb};
    let owner = SessionObservation::new(address());
    owner.send(Msg::Agent(AgentEvent::Text("intro".into())));
    owner.send(Msg::Agent(AgentEvent::ToolCall {
        id: "build".into(),
        title: "Build".into(),
        verb: ToolVerb::Run,
        status: ToolStatus::InProgress,
        diff: None,
        images: Vec::new(),
        record: Box::new(ToolRecord {
            input: Some(ToolPayload::Text("cargo check\r\n".into())),
            ..ToolRecord::default()
        }),
    }));
    owner.send(Msg::Agent(AgentEvent::Text(
        "reading below the tool".into(),
    )));
    let before = owner.snapshot();
    owner.send(Msg::Agent(AgentEvent::ToolCallUpdate {
        id: "build".into(),
        title: None,
        status: Some(ToolStatus::Failed),
        diff: None,
        duration_ms: Some(5),
        images: Vec::new(),
        record: Box::new(ToolRecord {
            text: vec!["exact error 日本".into()],
            ..ToolRecord::default()
        }),
    }));
    let snapshot = owner.snapshot();
    assert_eq!(
        snapshot
            .messages
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    let changed = snapshot.since(before.sequence).unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!((changed[0].id, changed[0].sequence), (2, 4));
    assert_eq!(snapshot.replay()[1], changed[0].message);
    let Msg::Agent(AgentEvent::ToolCallUpdate {
        record,
        title,
        status,
        ..
    }) = &changed[0].message
    else {
        panic!("tool");
    };
    assert_eq!(title.as_deref(), Some("Build"));
    assert_eq!(*status, Some(ToolStatus::Failed));
    assert_eq!(record.input.as_ref().unwrap().content(), "cargo check\r\n");
    assert_eq!(record.text, ["exact error 日本"]);
    assert_eq!(record.kind, Some(ToolVerb::Run));
    for _ in 0..MAX_MESSAGES {
        owner.send(Msg::Agent(AgentEvent::Notice("later".into())));
    }
    let pruned = owner.snapshot();
    assert!(pruned.since(before.sequence).is_none());
    assert!(pruned.dropped > 0);
    assert!(!pruned.messages.iter().any(|entry| entry.id == 2));
}

#[test]
fn an_existing_tool_revision_preserves_a_streamed_reply_across_delivery() {
    use magistr_application::sessions::{ToolRecord, ToolStatus, ToolVerb};
    let owner = SessionObservation::new(address());
    owner.send(Msg::Agent(AgentEvent::ToolCall {
        id: "build".into(),
        title: "Build".into(),
        verb: ToolVerb::Run,
        status: ToolStatus::InProgress,
        diff: None,
        images: Vec::new(),
        record: Box::new(ToolRecord::default()),
    }));
    owner.send(Msg::Agent(AgentEvent::Text("```rust\nlet ".into())));
    let before = owner.snapshot();
    owner.send(Msg::Agent(AgentEvent::ToolCallUpdate {
        id: "build".into(),
        title: None,
        status: Some(ToolStatus::Completed),
        diff: None,
        duration_ms: Some(5),
        images: Vec::new(),
        record: Box::new(ToolRecord::default()),
    }));
    owner.send(Msg::Agent(AgentEvent::Text("x = 1;\n```".into())));
    let snapshot = owner.snapshot();
    assert_eq!(snapshot.messages.len(), 2);
    let changed = snapshot.since(before.sequence).unwrap();
    assert_eq!(
        changed.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(changed[1].sequence, 4);
    assert_eq!(
        changed[1].message,
        Msg::Agent(AgentEvent::Text("```rust\nlet x = 1;\n```".into()))
    );
    assert_eq!(snapshot.replay()[1], changed[1].message);
    assert_eq!(snapshot.messages[1], changed[1]);
}

#[test]
fn a_visible_expired_request_keeps_subsequent_reply_source_after_the_notice() {
    let owner = SessionObservation::new(address());
    owner.send(Msg::Agent(AgentEvent::Text("before".into())));
    let notice = Msg::Agent(AgentEvent::PermissionLapsed {
        id: 7,
        title: "Read a file".into(),
        after_secs: 120,
    });
    owner.send(notice.clone());
    owner.send(Msg::Agent(AgentEvent::Text("after".into())));
    let snapshot = owner.snapshot();
    assert_eq!(
        snapshot.replay(),
        [
            Msg::Agent(AgentEvent::Text("before".into())),
            notice,
            Msg::Agent(AgentEvent::Text("after".into())),
        ]
    );
    let changed = snapshot.since(1).unwrap();
    assert_eq!(
        changed.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        [2, 3]
    );
}

#[test]
fn an_evicted_oversized_reply_never_reappears_as_a_fenced_source_tail() {
    let owner = SessionObservation::new(address());
    owner.send(Msg::Agent(AgentEvent::Text("```rust\n".into())));
    owner.send(Msg::Agent(AgentEvent::Text("x".repeat(MAX_BYTES))));
    assert_eq!(owner.snapshot().dropped, 1);
    owner.send(Msg::Agent(AgentEvent::Text(
        "\n```\narbitrary suffix".into(),
    )));
    let pruned = owner.snapshot();
    assert!(pruned.messages.is_empty());
    assert_eq!(pruned.bytes, 0);
    assert_eq!(pruned.dropped, 1);
    assert!(pruned.since(1).is_none());
    assert!(pruned.since(2).unwrap().is_empty());
    owner.send(Msg::Agent(AgentEvent::Thought("complete thought".into())));
    assert_eq!(owner.snapshot().messages.len(), 1);
    owner.send(Msg::Agent(AgentEvent::TurnEnded(
        magistr_application::sessions::TurnEnd::EndTurn,
    )));
    owner.send(Msg::Agent(AgentEvent::Text("complete next reply".into())));
    let recovered = owner.snapshot();
    assert!(recovered.bytes <= MAX_BYTES);
    assert_eq!(
        recovered.messages.last().unwrap().message,
        Msg::Agent(AgentEvent::Text("complete next reply".into()))
    );
}

#[test]
fn ancillary_record_pressure_cannot_turn_the_active_reply_tail_into_a_new_message() {
    use magistr_application::sessions::{ToolRecord, ToolStatus, ToolVerb};
    let owner = SessionObservation::new(address());
    owner.send(Msg::Agent(AgentEvent::Text("beginning".into())));
    for _ in 0..MAX_MESSAGES {
        owner.send(Msg::Agent(AgentEvent::Usage {
            context: None,
            cost_cents: Some(0),
            elapsed_secs: None,
        }));
    }
    let pruned = owner.snapshot();
    assert_eq!(pruned.dropped, 1);
    owner.send(Msg::Agent(AgentEvent::Text("unretained tail".into())));
    assert!(owner.snapshot().since(pruned.sequence).unwrap().is_empty());
    assert!(
        !owner
            .snapshot()
            .messages
            .iter()
            .any(|entry| matches!(entry.message, Msg::Agent(AgentEvent::Text(_))))
    );
    owner.send(Msg::Agent(AgentEvent::ToolCall {
        id: "next".into(),
        title: "New activity".into(),
        verb: ToolVerb::Run,
        status: ToolStatus::InProgress,
        diff: None,
        images: Vec::new(),
        record: Box::new(ToolRecord::default()),
    }));
    owner.send(Msg::Agent(AgentEvent::Text("complete later reply".into())));
    let recovered = owner.snapshot();
    assert!(recovered.bytes <= MAX_BYTES);
    assert!(recovered.messages.len() <= MAX_MESSAGES);
    assert_eq!(
        recovered.messages.last().unwrap().message,
        Msg::Agent(AgentEvent::Text("complete later reply".into()))
    );
}

#[test]
fn streamed_source_survives_reseed_and_changed_record_delivery() {
    let owner = SessionObservation::new(address());
    for _ in 0..700 {
        owner.send(Msg::Agent(AgentEvent::Text("日本\r\n".into())));
    }
    let snapshot = owner.snapshot();
    assert_eq!(snapshot.messages.len(), 1);
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(
        (snapshot.messages[0].id, snapshot.messages[0].sequence),
        (1, 700)
    );
    assert_eq!(
        snapshot.since(699).unwrap()[0].message,
        Msg::Agent(AgentEvent::Text("日本\r\n".repeat(700)))
    );
    owner.send(Msg::Agent(AgentEvent::TurnEnded(
        magistr_application::sessions::TurnEnd::EndTurn,
    )));
    owner.send(Msg::Agent(AgentEvent::Text("next reply".into())));
    assert_ne!(owner.snapshot().messages.last().unwrap().id, 1);
}

#[test]
fn conversation_name_and_complete_configuration_survive_transcript_pruning() {
    let owner = SessionObservation::new(address());
    owner.send(Msg::Agent(AgentEvent::Conversation(
        magistr_application::sessions::Conversation {
            id: "saved".into(),
            name: Some("Review café".into()),
            provider: "copilot".into(),
            purpose: magistr_application::sessions::SessionPurpose::Conversation,
        },
    )));
    owner.send(Msg::Agent(AgentEvent::Configuration {
        model: Some("confirmed".into()),
        options: vec![magistr_application::sessions::SessionConfig {
            id: "model".into(),
            name: "Model".into(),
            current_value: "confirmed".into(),
            values: vec![("confirmed".into(), "Confirmed".into())],
        }],
    }));
    for _ in 0..MAX_MESSAGES + 2 {
        owner.send(Msg::Agent(AgentEvent::Notice("reply".into())));
    }
    let snapshot = owner.snapshot();
    assert!(snapshot.dropped > 0);
    assert_eq!(
        snapshot.conversation.unwrap().name.as_deref(),
        Some("Review café")
    );
    assert_eq!(snapshot.model.as_deref(), Some("confirmed"));
    assert_eq!(snapshot.configuration[0].current_value, "confirmed");
}
#[test]
fn submission_outcome_survives_history_pruning_without_retaining_attachment_bytes() {
    use magistr_application::sessions::{MAX_ATTACHMENT_BYTES, PromptAttachment};
    let owner = SessionObservation::new(address());
    let input = PromptInput {
        text: "image".into(),
        attachments: vec![
            PromptAttachment::from_bytes(
                "picture.png",
                "exact-hash",
                [
                    b"\x89PNG\r\n\x1a\n".as_slice(),
                    &vec![1; MAX_ATTACHMENT_BYTES - 8],
                ]
                .concat(),
            )
            .unwrap(),
        ],
    };
    owner.send(Msg::InputAccepted(input.receipt()));
    let accepted = owner.snapshot().admission.unwrap();
    assert!(accepted.accepted);
    for _ in 0..MAX_MESSAGES + 1 {
        owner.send(Msg::Agent(AgentEvent::Notice("later".into())));
    }
    assert_eq!(owner.snapshot().admission.as_ref(), Some(&accepted));
    owner.send(Msg::Agent(AgentEvent::PromptRejected {
        input: input.clone(),
        error: "Try again".into(),
    }));
    let refused = owner.snapshot();
    assert!(!refused.admission.as_ref().unwrap().accepted);
    assert!(refused.admission.as_ref().unwrap().sequence > accepted.sequence);
    assert_eq!(
        refused.attachments[0].content().bytes(),
        input.attachments[0].bytes()
    );
    assert!(refused.messages.iter().all(|record| !matches!(
        record.message,
        Msg::Agent(AgentEvent::PromptRejected { .. })
    )));
    assert!(
        matches!(&refused.messages.last().unwrap().message, Msg::Agent(AgentEvent::Notice(error)) if error == "Try again")
    );
}

#[test]
fn workflow_snapshots_survive_separately_without_evicting_conversation_or_drafts() {
    use magistr_cockpit::{engine::EngineEvent, harness::fixtures};
    use magistr_engine::{ReadError, ReadErrorKind, ReadState};
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    bind(&owner);
    owner.draft_for("one", "unsent message".into()).unwrap();
    owner.send(Msg::Agent(AgentEvent::UserText("earlier question".into())));
    owner.send(Msg::Agent(AgentEvent::PermissionRequest {
        detail: String::new(),
        id: 7,
        title: "Read".into(),
        choices: Vec::new(),
        edit: false,
        risk: magistr_domain::policy::RiskTier::Low,
        plan: None,
    }));
    let before = owner.snapshot();
    let mut chunk = fixtures::chunk("selected-record");
    chunk.description = "x".repeat(MAX_BYTES + 1);
    let snapshot = fixtures::snapshot(vec![chunk], Vec::new());
    owner.send(Msg::Engine(EngineEvent::Seed(snapshot.clone())));
    let revision = owner.snapshot().workflow_revision;
    assert_eq!(revision, before.workflow_revision + 1);
    owner.send(Msg::Engine(EngineEvent::Seed(snapshot.clone())));
    assert_eq!(owner.snapshot().workflow_revision, revision);
    let failed = snapshot.failed(ReadError::new(
        ReadErrorKind::Io,
        "Cannot read work records.",
    ));
    owner.send(Msg::Engine(EngineEvent::Seed(failed.clone())));
    let after = owner.snapshot();
    assert_eq!(after.address, before.address);
    assert_eq!(after.messages, before.messages);
    assert_eq!(after.pending, before.pending);
    assert_eq!(after.dropped, before.dropped);
    assert_eq!(after.seed.draft.as_deref(), Some("unsent message"));
    assert_eq!(after.seed.snapshot.as_ref(), &failed);
    assert_eq!(after.workflow_revision, revision + 1);
    for _ in 0..128 {
        owner.send(Msg::Agent(AgentEvent::Text("another reply".into())));
        let frame = owner.snapshot();
        assert_eq!(frame.workflow_revision, after.workflow_revision);
        assert!(std::sync::Arc::ptr_eq(
            &after.seed.snapshot,
            &frame.seed.snapshot
        ));
    }
    let mut next = owner.snapshot().conversation.unwrap();
    next.id = "two".into();
    owner.bind_context_for_test(next);
    assert_eq!(owner.snapshot().workflow_revision, after.workflow_revision);
    owner.activate_context("one").unwrap();
    assert_eq!(owner.snapshot().workflow_revision, after.workflow_revision);
    assert!(matches!(
        after.seed.snapshot.roadmap,
        ReadState::Stale { .. }
    ));
    assert_eq!(
        after
            .seed
            .snapshot
            .roadmap
            .last_successful()
            .unwrap()
            .value
            .chunks[0]
            .id,
        "selected-record"
    );
    assert!(
        !after
            .replay()
            .iter()
            .any(|message| matches!(message, Msg::Engine(_)))
    );
}

#[tokio::test]
async fn duplicate_events_do_not_advance_sequence_or_freshness_and_close_refresh_returns() {
    use magistr_cockpit::{engine::EngineEvent, harness::fixtures};
    use magistr_engine::{Delivery, NativeConfig, NativeEvent, NativeReader, RoadmapFrame};
    let temp = tempfile::tempdir().unwrap();
    let config = NativeConfig::from_lookup(temp.path().to_owned(), |_| None)
        .with_data_dir(temp.path().join("data"));
    let reads = super::super::workflow_client::SnapshotReads::new(NativeReader::open_for_project(
        &config,
        "observed".into(),
    ));
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    owner.set_reader(reads, super::super::workflow_client::FeedLink::new());
    owner.send(Msg::Engine(EngineEvent::Seed(fixtures::snapshot(
        vec![fixtures::chunk("selected")],
        vec![],
    ))));
    let event = NativeEvent::Roadmap(RoadmapFrame::ChunkChanged {
        chunk: fixtures::chunk("other"),
    });
    owner.send(Msg::Engine(EngineEvent::Event {
        event: event.clone(),
        delivery: Delivery::InOrder,
    }));
    let before = owner.snapshot();
    owner.send(Msg::Engine(EngineEvent::Event {
        event,
        delivery: Delivery::Duplicate,
    }));
    assert_eq!(owner.snapshot(), before);
    owner.stop();
    let mut subscription = owner.subscribe();
    let snapshot = tokio::time::timeout(std::time::Duration::from_secs(1), subscription.refresh())
        .await
        .unwrap()
        .unwrap();
    assert!(snapshot.closed);
    assert!(snapshot.seed.snapshot.roadmap.current().is_err());
}

#[test]
fn actual_work_and_launch_refusal_survive_pruning_reseed_and_conversation_switches() {
    use magistr_cockpit::cockpit::{LaunchAction, WorkPhase, WorkRun};
    use magistr_domain::{
        identity::{ProjectId, RunId},
        workflow::RunMode,
    };
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    bind(&owner);
    let mut run = WorkRun {
        id: RunId::new(ProjectId::new("project").unwrap(), "run-one").unwrap(),
        mode: RunMode::ChunkLoop,
        track: None,
        chunk: Some("item".into()),
        iteration: 1,
        limit: 10,
        phase: WorkPhase::Starting,
        requested: None,
    };
    owner.send(Msg::CommandFailed {
        cmd: magistr_cockpit::cockpit::Cmd::Launch(LaunchAction::Next),
        error: "Work is already running.".into(),
    });
    assert!(
        owner.snapshot().work.is_none(),
        "refusal never starts a run"
    );
    assert!(owner.snapshot().work_admission.unwrap().run_id.is_none());
    owner.send(Msg::WorkRun(run.clone()));
    let admitted = owner.snapshot().work_admission.unwrap();
    assert_eq!(admitted.run_id.as_ref(), Some(&run.id));
    run.phase = WorkPhase::Running;
    owner.send(Msg::WorkRun(run.clone()));
    for _ in 0..MAX_MESSAGES + 3 {
        owner.send(Msg::Agent(AgentEvent::Notice("trim history".into())));
    }
    assert!(owner.snapshot().since(0).is_none());
    assert_eq!(owner.subscribe().snapshot().work.as_ref(), Some(&run));
    owner.bind_context_for_test(magistr_application::sessions::Conversation {
        id: "other".into(),
        name: None,
        provider: "scripted".into(),
        purpose: magistr_application::sessions::SessionPurpose::Conversation,
    });
    run.phase = WorkPhase::Paused;
    owner.send(Msg::WorkRun(run.clone()));
    owner.activate_context("one").unwrap();
    assert_eq!(owner.snapshot().work.as_ref(), Some(&run));
    assert_eq!(owner.snapshot().work_admission, Some(admitted));
    run.phase = WorkPhase::Ended {
        reason: "Finished.".into(),
    };
    owner.send(Msg::WorkRun(run.clone()));
    assert_eq!(owner.context_snapshot("other").unwrap().work, Some(run));
}
