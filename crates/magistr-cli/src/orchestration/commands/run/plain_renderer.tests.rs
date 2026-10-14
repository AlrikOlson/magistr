use super::*;
use crate::agent::message::{AssistantContent, SessionId};
use serde_json::json;

#[test]
fn direct_tool_errors_remain_visible_without_stream_updates() {
    let mut renderer = PlainRenderer::default();
    let mut out = Vec::new();
    let mut err = Vec::new();
    renderer
        .render(&tool("write", "Edit", json!({})), &mut out, &mut err)
        .unwrap();
    renderer
        .render(
            &assistant(ContentBlock::ToolResult {
                tool_use_id: "write".into(),
                content: Some("writer refused".into()),
                is_error: Some(true),
            }),
            &mut out,
            &mut err,
        )
        .unwrap();
    let text = String::from_utf8(err).unwrap();
    assert!(text.contains("writer refused"));
    assert!(text.ends_with("[failed] Edit\n"));
}

#[test]
fn partial_output_does_not_split_words_and_status_only_completion_flushes_it() {
    let mut renderer = PlainRenderer::default();
    let mut out = Vec::new();
    let mut err = Vec::new();
    for msg in [
        tool("command", "check_with_underscores", json!({})),
        update("command", "", "Che"),
        update("command", "", "Checking"),
        update("command", "Completed", ""),
    ] {
        renderer.render(&msg, &mut out, &mut err).unwrap();
    }
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "[tool] check_with_underscores\n  Checking\n[done] check_with_underscores\n"
    );
}

#[test]
fn raw_terminal_bytes_survive_chunk_boundaries_and_end_before_the_next_tool() {
    let mut renderer = PlainRenderer::default();
    let mut out = Vec::new();
    let mut err = Vec::new();
    for bytes in [&b"hello "[..], &b"world"[..]] {
        renderer
            .render(
                &AgentMessage::System {
                    subtype: "terminal_output".into(),
                    data: json!({"terminal_id":"command","bytes":bytes}),
                },
                &mut out,
                &mut err,
            )
            .unwrap();
    }
    renderer
        .render(&tool("next", "Read", json!({})), &mut out, &mut err)
        .unwrap();
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "hello world\n[tool] Read\n"
    );
}

fn assistant(block: ContentBlock) -> AgentMessage {
    AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "test".into(),
            content: vec![block],
        },
        session_id: None,
    }
}
fn tool(id: &str, name: &str, input: Value) -> AgentMessage {
    assistant(ContentBlock::ToolUse {
        id: id.into(),
        name: name.into(),
        input,
        kind: None,
        locations: vec![],
        tool_name: None,
    })
}
fn update(id: &str, status: &str, content: &str) -> AgentMessage {
    AgentMessage::StreamEvent {
        uuid: id.into(),
        session_id: SessionId::new("terminal"),
        parent_tool_use_id: None,
        event: json!({"type":"tool_call_update","tool_call_id":id,"status":status,"content":content}),
    }
}

#[test]
fn cumulative_output_prints_each_line_once_and_keeps_failures() {
    let mut renderer = PlainRenderer::default();
    let mut out = Vec::new();
    let mut err = Vec::new();
    for msg in [
        tool("command", "cargo check", json!({})),
        update("command", "InProgress", "Checking one\n"),
        update("command", "InProgress", "Checking one\nChecking two\n"),
        update(
            "command",
            "Failed",
            "Checking one\nChecking two\nerror: invalid type\n",
        ),
    ] {
        renderer.render(&msg, &mut out, &mut err).unwrap();
    }
    let text = String::from_utf8(err).unwrap();
    assert_eq!(text.matches("Checking one").count(), 1);
    assert_eq!(text.matches("Checking two").count(), 1);
    assert!(text.contains("error: invalid type"));
    assert!(text.ends_with("[failed] cargo check\n"));
}

#[test]
fn findings_and_phases_are_concise_and_long_results_show_their_tail() {
    let mut renderer = PlainRenderer::default();
    let mut out = Vec::new();
    let mut err = Vec::new();
    renderer
        .render(
            &tool(
                "phase",
                "mcp__magistr__magistr_set_phase",
                json!({"phase":"verify"}),
            ),
            &mut out,
            &mut err,
        )
        .unwrap();
    renderer.render(&tool("finding", "mcp__magistr__think_record_step", json!({"purpose":"Retain spending","outcome":"The owner keeps the reported amount.","thought":"private record detail","estimated_total":6})), &mut out, &mut err).unwrap();
    let lines = (0..30)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    renderer
        .render(
            &tool("long", "Read", json!({"file":"file.rs"})),
            &mut out,
            &mut err,
        )
        .unwrap();
    renderer
        .render(&update("long", "Completed", &lines), &mut out, &mut err)
        .unwrap();
    let text = String::from_utf8(err).unwrap();
    assert!(text.contains("Phase: verify"));
    assert!(text.contains("Finding: Retain spending\n  The owner keeps the reported amount."));
    assert!(!text.contains("private record detail"));
    assert!(!text.contains("estimated_total"));
    assert!(text.contains("18 lines omitted"));
    assert!(text.contains("line 29"));
}

#[derive(Clone)]
struct OrderedWriter(std::rc::Rc<std::cell::RefCell<Vec<String>>>, &'static str);
impl Write for OrderedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .borrow_mut()
            .push(format!("{}:{}", self.1, String::from_utf8_lossy(bytes)));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.borrow_mut().push(format!("{}:flush", self.1));
        Ok(())
    }
}

#[test]
fn partial_assistant_text_is_flushed_and_ended_before_tool_activity() {
    let recorded = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut out = OrderedWriter(recorded.clone(), "out");
    let mut err = OrderedWriter(recorded.clone(), "err");
    let mut renderer = PlainRenderer::default();
    renderer
        .render(
            &assistant(ContentBlock::Text {
                text: "Starting".into(),
            }),
            &mut out,
            &mut err,
        )
        .unwrap();
    renderer
        .render(
            &tool("read", "Read", json!({"file":"file.rs"})),
            &mut out,
            &mut err,
        )
        .unwrap();
    renderer
        .render(
            &assistant(ContentBlock::Text {
                text: "Finished.".into(),
            }),
            &mut out,
            &mut err,
        )
        .unwrap();
    let events = recorded.borrow();
    let start = events.iter().position(|v| v == "out:Starting").unwrap();
    let flush = events.iter().position(|v| v == "out:flush").unwrap();
    let newline = events.iter().position(|v| v == "out:\n").unwrap();
    let tool = events.iter().position(|v| v.contains("[tool]")).unwrap();
    let final_text = events.iter().position(|v| v == "out:Finished.").unwrap();
    assert!(
        start < flush && flush < newline && newline < tool && tool < final_text,
        "{events:?}"
    );
}
