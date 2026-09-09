use agent_client_protocol::schema::v1::{
    AgentCapabilities, AudioContent, ContentBlock, ContentChunk, Cost, ImageContent, Plan,
    PlanEntry, PlanEntryPriority, PlanEntryStatus, PromptResponse, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigSelectOption, SessionUpdate, StopReason, TextContent,
    ToolCall, ToolCallId, ToolCallStatus, ToolCallUpdate, UsageUpdate,
};

use super::*;

fn update(update: SessionUpdate) -> magistr_acp::Result<AcpEvent> {
    Ok(AcpEvent::SessionUpdate {
        update: Box::new(update),
        session_id: "s1".to_owned(),
    })
}

fn chunk(text: &str) -> ContentChunk {
    ContentChunk::new(ContentBlock::Text(TextContent::new(text)))
}

#[test]
fn message_thought_and_user_chunks_carry_their_text() {
    assert_eq!(
        translate(&update(SessionUpdate::AgentMessageChunk(chunk("hi")))),
        AgentEvent::Text("hi".to_owned())
    );
    assert_eq!(
        translate(&update(SessionUpdate::AgentThoughtChunk(chunk("hmm")))),
        AgentEvent::Thought("hmm".to_owned())
    );
    assert_eq!(
        translate(&update(SessionUpdate::UserMessageChunk(chunk("do it")))),
        AgentEvent::UserText("do it".to_owned())
    );
}

#[test]
fn tool_calls_and_their_updates_keep_id_title_and_status() {
    let call = ToolCall::new(ToolCallId::new("tc-1"), "Read main.rs")
        .kind(ToolKind::Read)
        .status(ToolCallStatus::InProgress);
    assert_eq!(
        translate(&update(SessionUpdate::ToolCall(call.clone()))),
        AgentEvent::ToolCall {
            id: "tc-1".to_owned(),
            title: "Read main.rs".to_owned(),
            verb: ToolVerb::Read,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                kind: Some(ToolVerb::Read),
                ..ToolRecord::default()
            }),
        }
    );
    let done = ToolCallUpdate::from(call.status(ToolCallStatus::Completed));
    assert_eq!(
        translate(&update(SessionUpdate::ToolCallUpdate(done))),
        AgentEvent::ToolCallUpdate {
            id: "tc-1".to_owned(),
            title: Some("Read main.rs".to_owned()),
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                kind: Some(ToolVerb::Read),
                ..ToolRecord::default()
            }),
        }
    );
}

#[test]
fn a_call_keeps_its_input_output_text_locations_and_terminal_capped() {
    use agent_client_protocol::schema::v1::{Terminal, ToolCallLocation, ToolCallUpdateFields};
    let call = ToolCall::new(ToolCallId::new("tc-9"), "Run cargo test")
        .kind(ToolKind::Execute)
        .status(ToolCallStatus::InProgress)
        .raw_input(serde_json::json!({"command": "cargo test"}))
        .locations(vec![
            ToolCallLocation::new("/repo/src/lib.rs").line(12u32),
            ToolCallLocation::new("/repo/Cargo.toml"),
        ])
        .content(vec![
            ToolCallContent::from(ContentBlock::Text(TextContent::new("running 3 tests"))),
            ToolCallContent::Terminal(Terminal::new("term-4")),
        ]);
    let AgentEvent::ToolCall { record, .. } =
        translate(&update(SessionUpdate::ToolCall(call.clone())))
    else {
        panic!("not a tool call");
    };
    assert_eq!(
        *record,
        ToolRecord {
            kind: Some(ToolVerb::Run),
            input: Some(serde_json::json!({"command": "cargo test"})),
            output: None,
            text: vec!["running 3 tests".to_owned()],
            locations: vec![
                (std::path::PathBuf::from("/repo/src/lib.rs"), Some(12)),
                (std::path::PathBuf::from("/repo/Cargo.toml"), None),
            ],
            terminal: Some("term-4".to_owned()),
            truncated_from: None,
        }
    );

    // An update carries only what it says; a raw output past the cap
    // arrives cut, its size noted.
    let output = "x".repeat(ToolRecord::CAP * 2);
    let done = ToolCallUpdate::new(
        ToolCallId::new("tc-9"),
        ToolCallUpdateFields::new()
            .status(ToolCallStatus::Completed)
            .raw_output(serde_json::Value::String(output)),
    );
    let AgentEvent::ToolCallUpdate { record, .. } =
        translate(&update(SessionUpdate::ToolCallUpdate(done)))
    else {
        panic!("not a tool call update");
    };
    assert_eq!(record.kind, None);
    assert_eq!(record.input, None);
    assert!(record.text.is_empty() && record.locations.is_empty());
    assert_eq!(record.truncated_from, Some(ToolRecord::CAP * 2));
    assert_eq!(record.size(), ToolRecord::CAP);
}

#[test]
fn plans_usage_and_the_turn_end_are_summarised() {
    let plan = Plan::new(vec![
        PlanEntry::new("a", PlanEntryPriority::High, PlanEntryStatus::Completed),
        PlanEntry::new("b", PlanEntryPriority::Low, PlanEntryStatus::Pending),
        PlanEntry::new("c", PlanEntryPriority::Low, PlanEntryStatus::InProgress),
    ]);
    assert_eq!(
        translate(&update(SessionUpdate::Plan(plan))),
        AgentEvent::Plan {
            entries: 3,
            completed: 1
        }
    );
    assert_eq!(
        translate(&update(SessionUpdate::UsageUpdate(UsageUpdate::new(
            1_000, 200_000
        )))),
        AgentEvent::Usage {
            used: 1_000,
            size: 200_000,
            cost_cents: None,
            elapsed_secs: None,
        }
    );
    assert_eq!(
        translate(&Ok(AcpEvent::PromptResponse {
            response: PromptResponse::new(StopReason::Cancelled),
            session_id: "s1".to_owned(),
        })),
        AgentEvent::TurnEnded(TurnEnd::Cancelled)
    );
}

/// the cost the agent puts on the usage update
/// is the session's cumulative spend — the same `total_cost_usd` the
/// ledger is written from — and it reaches the cockpit in cents.
#[test]
fn the_usage_updates_cost_arrives_in_cents_when_it_is_dollars() {
    let priced = UsageUpdate::new(1_000, 200_000).cost(Cost::new(1.844, "USD"));
    assert_eq!(
        translate(&update(SessionUpdate::UsageUpdate(priced))),
        AgentEvent::Usage {
            used: 1_000,
            size: 200_000,
            cost_cents: Some(184),
            elapsed_secs: None,
        }
    );
    // A cap in dollars cannot be metered in euros: another currency is
    // an unmetered spend, never a number in the wrong unit.
    let foreign = UsageUpdate::new(1_000, 200_000).cost(Cost::new(1.844, "EUR"));
    assert_eq!(
        translate(&update(SessionUpdate::UsageUpdate(foreign))),
        AgentEvent::Usage {
            used: 1_000,
            size: 200_000,
            cost_cents: None,
            elapsed_secs: None,
        }
    );
}

#[test]
fn a_terminals_bytes_ride_whole_by_id() {
    assert_eq!(
        translate(&Ok(AcpEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: b"\x1b[31mred\x1b[0m\r\n".to_vec(),
        })),
        AgentEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: b"\x1b[31mred\x1b[0m\r\n".to_vec(),
        }
    );
}

#[test]
fn system_events_and_errors_keep_their_names() {
    assert_eq!(
        translate(&Ok(AcpEvent::System {
            subtype: "permission_request".to_owned(),
            data: serde_json::Value::Null,
        })),
        AgentEvent::System {
            subtype: "permission_request".to_owned()
        }
    );
    let err: magistr_acp::Result<AcpEvent> =
        Err(magistr_acp::AcpError::Transport("bad frame".to_owned()));
    assert!(matches!(translate(&err), AgentEvent::Error(text) if text.contains("bad frame")));
}

#[test]
fn a_diff_content_block_rides_the_call_as_an_edit_diff() {
    use crate::agent::EditDiff;
    use agent_client_protocol::schema::v1::{Diff, ToolCallContent, ToolKind};
    let call = ToolCall::new(ToolCallId::new("tc-2"), "Edit main.rs")
        .kind(ToolKind::Edit)
        .status(ToolCallStatus::InProgress)
        .content(vec![ToolCallContent::Diff(
            Diff::new("/repo/src/main.rs", "new\n").old_text("old\n".to_owned()),
        )]);
    assert_eq!(
        translate(&update(SessionUpdate::ToolCall(call.clone()))),
        AgentEvent::ToolCall {
            id: "tc-2".to_owned(),
            title: "Edit main.rs".to_owned(),
            verb: ToolVerb::Edit,
            status: ToolStatus::InProgress,
            diff: Some(EditDiff {
                path: "/repo/src/main.rs".to_owned(),
                old_text: "old\n".to_owned(),
                new_text: "new\n".to_owned(),
            }),
            images: Vec::new(),
            record: Box::new(ToolRecord {
                kind: Some(ToolVerb::Edit),
                ..ToolRecord::default()
            }),
        }
    );
    // The same diff on an update; a missing old_text reads as empty.
    let done = ToolCallUpdate::from(call.status(ToolCallStatus::Completed).content(vec![
        ToolCallContent::Diff(Diff::new("/repo/src/main.rs", "new\n")),
    ]));
    let translated = translate(&update(SessionUpdate::ToolCallUpdate(done)));
    let AgentEvent::ToolCallUpdate {
        diff: Some(diff), ..
    } = translated
    else {
        panic!("no diff on the update: {translated:?}");
    };
    assert_eq!(diff.old_text, "", "a new file's old side is empty");
    assert_eq!(diff.new_text, "new\n");
}

#[test]
fn an_image_block_arrives_as_its_bytes_and_type_in_every_voice() {
    let image = ContentChunk::new(ContentBlock::Image(ImageContent::new("aGk=", "image/png")));
    let expected = AgentEvent::Image(ImageBlock {
        mime_type: "image/png".to_owned(),
        data: "aGk=".to_owned(),
        uri: None,
    });
    assert_eq!(
        translate(&update(SessionUpdate::AgentMessageChunk(image.clone()))),
        expected
    );
    assert_eq!(
        translate(&update(SessionUpdate::AgentThoughtChunk(image.clone()))),
        expected
    );
    assert_eq!(
        translate(&update(SessionUpdate::UserMessageChunk(image))),
        expected,
        "the same block whichever voice sent it"
    );
}

#[test]
fn a_block_that_is_not_an_image_still_reads_as_its_bracketed_kind() {
    let audio = ContentChunk::new(ContentBlock::Audio(AudioContent::new("aGk=", "audio/wav")));
    assert_eq!(
        translate(&update(SessionUpdate::AgentMessageChunk(audio))),
        AgentEvent::Text("[audio]".to_owned())
    );
}

#[test]
fn an_image_on_a_tool_calls_content_rides_the_call_instead_of_being_dropped() {
    use agent_client_protocol::schema::v1::{Diff, ToolCallContent, ToolKind};
    let shot = ImageContent::new("aGk=", "image/png");
    let call = ToolCall::new(ToolCallId::new("tc-3"), "Take a screenshot")
        .kind(ToolKind::Fetch)
        .status(ToolCallStatus::Completed)
        .content(vec![
            ToolCallContent::from(ContentBlock::Text(TextContent::new("done"))),
            ToolCallContent::from(ContentBlock::Image(shot.clone())),
            ToolCallContent::Diff(Diff::new("/repo/a.rs", "new\n")),
            ToolCallContent::from(ContentBlock::Image(
                ImageContent::new("Ynll", "image/png").uri("file:///tmp/two.png".to_owned()),
            )),
        ]);
    let AgentEvent::ToolCall { images, diff, .. } =
        translate(&update(SessionUpdate::ToolCall(call.clone())))
    else {
        panic!("not a tool call");
    };
    assert!(diff.is_some(), "the diff still reaches the edit card");
    assert_eq!(
        images,
        vec![
            ImageBlock {
                mime_type: "image/png".to_owned(),
                data: "aGk=".to_owned(),
                uri: None,
            },
            ImageBlock {
                mime_type: "image/png".to_owned(),
                data: "Ynll".to_owned(),
                uri: Some("file:///tmp/two.png".to_owned()),
            },
        ],
        "every image block, in the order the call listed them"
    );

    // The same list on an update; a call that carries no content at all
    // reports no images rather than clearing what the reader has seen.
    let done =
        ToolCallUpdate::from(call.content(vec![ToolCallContent::from(ContentBlock::Image(shot))]));
    let AgentEvent::ToolCallUpdate { images, .. } =
        translate(&update(SessionUpdate::ToolCallUpdate(done)))
    else {
        panic!("not a tool call update");
    };
    assert_eq!(images.len(), 1);
    let bare = ToolCallUpdate::from(ToolCall::new(ToolCallId::new("tc-3"), "Take a screenshot"));
    let AgentEvent::ToolCallUpdate { images, .. } =
        translate(&update(SessionUpdate::ToolCallUpdate(bare)))
    else {
        panic!("not a tool call update");
    };
    assert!(images.is_empty());
}

// ── The handshake's model ──────────────

fn initialized(config_options: Option<Vec<SessionConfigOption>>) -> magistr_acp::Result<AcpEvent> {
    Ok(AcpEvent::Initialized {
        agent_capabilities: Box::new(AgentCapabilities::new()),
        session_id: "s1".to_owned(),
        config_options,
        agent_meta: None,
    })
}

#[test]
fn the_handshake_names_the_model_the_session_opened_on() {
    let picker = SessionConfigOption::select(
        "model",
        "Model",
        "sonnet",
        vec![
            SessionConfigSelectOption::new("sonnet", "Sonnet"),
            SessionConfigSelectOption::new("haiku", "Haiku"),
        ],
    )
    .category(SessionConfigOptionCategory::Model);
    assert_eq!(
        translate(&initialized(Some(vec![picker]))),
        AgentEvent::Initialized {
            session_id: "s1".to_owned(),
            model: Some("sonnet".to_owned()),
        }
    );
    // An agent that advertises no model option names none: the rail's
    // block stays unbound rather than guessing.
    let other = SessionConfigOption::select(
        "mode",
        "Mode",
        "ask",
        Vec::<SessionConfigSelectOption>::new(),
    )
    .category(SessionConfigOptionCategory::Mode);
    assert_eq!(
        translate(&initialized(Some(vec![other]))),
        AgentEvent::Initialized {
            session_id: "s1".to_owned(),
            model: None,
        }
    );
    assert_eq!(
        translate(&initialized(None)),
        AgentEvent::Initialized {
            session_id: "s1".to_owned(),
            model: None,
        }
    );
}
