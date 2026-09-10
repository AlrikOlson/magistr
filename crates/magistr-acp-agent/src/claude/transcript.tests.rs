#[path = "../../../magistr-cli/tests/support/in_env.rs"]
mod in_env;

use super::*;

const TRANSCRIPT: &str = include_str!("../../tests/fixtures/claude/transcript.jsonl");
const SESSION: &str = "00000000-0000-4000-8000-0000000001e3";

fn text_of(update: &SessionUpdate) -> (&'static str, String) {
    let (role, chunk) = match update {
        SessionUpdate::UserMessageChunk(c) => ("user", c),
        SessionUpdate::AgentMessageChunk(c) => ("agent", c),
        other => panic!("unexpected replay update {other:?}"),
    };
    match &chunk.content {
        AcpContent::Text(t) => (role, t.text.clone()),
        other => panic!("unexpected content {other:?}"),
    }
}

#[test]
fn the_path_encodes_the_cwd_with_dashes_under_projects() {
    let path = transcript_path(
        Path::new("/home/me/.claude"),
        Path::new("/private/tmp/scratch/probe/work"),
        SESSION,
    );
    assert_eq!(
        path,
        PathBuf::from(format!(
            "/home/me/.claude/projects/-private-tmp-scratch-probe-work/{SESSION}.jsonl"
        ))
    );
}

#[test]
fn the_recorded_transcript_replays_prompts_answers_and_thinking() {
    let config = tempfile::tempdir().expect("tempdir");
    let cwd = Path::new("/private/tmp/scratch/probe/work");
    let path = transcript_path(config.path(), cwd, SESSION);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, TRANSCRIPT).expect("write");

    let updates = replay(config.path(), cwd, SESSION).expect("complete transcript");
    assert!(
        updates
            .iter()
            .any(|update| matches!(update, SessionUpdate::AgentThoughtChunk(_)))
    );
    let replayed: Vec<_> = updates
        .iter()
        .filter(|update| !matches!(update, SessionUpdate::AgentThoughtChunk(_)))
        .map(text_of)
        .collect();
    assert_eq!(replayed.len(), 6, "{replayed:?}");
    assert_eq!(replayed[0].0, "user");
    assert!(
        replayed[0].1.starts_with("Think for a moment"),
        "{}",
        replayed[0].1
    );
    assert_eq!(replayed[1], ("agent", "pong".to_owned()));
    assert_eq!(
        replayed[2],
        ("user", "Reply with exactly one word: ping".to_owned())
    );
    assert_eq!(replayed[3], ("agent", "ping".to_owned()));
    assert_eq!(
        replayed[5],
        ("agent", "pong".to_owned()),
        "text messages preserve their order around reasoning"
    );
}

#[test]
fn a_missing_transcript_refuses_load() {
    let config = tempfile::tempdir().expect("tempdir");
    assert!(replay(config.path(), Path::new("/nowhere"), SESSION).is_err());
}

#[test]
fn malformed_or_unsupported_history_cannot_be_partially_replayed() {
    let config = tempfile::tempdir().expect("tempdir");
    let cwd = Path::new("/fixture/project");
    let path = transcript_path(config.path(), cwd, SESSION);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    for malformed in [
        "{invalid",
        r#"{"type":"assistant","message":{"content":[{"type":"image"}]}}"#,
        r#"{"type":"user","sessionId":"another-session","message":{"content":"text"}}"#,
        r#"{"type":"assistant","message":{}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"text"}]}}"#,
        "",
    ] {
        std::fs::write(&path, malformed).expect("write");
        assert!(replay(config.path(), cwd, SESSION).is_err(), "{malformed}");
    }
}

#[test]
fn nonobject_records_refuse_an_otherwise_valid_transcript() {
    let config = tempfile::tempdir().expect("tempdir");
    let cwd = Path::new("/fixture/project");
    let path = transcript_path(config.path(), cwd, SESSION);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    let valid = r#"{"type":"user","message":{"content":"existing conversation"}}"#;
    for invalid in ["42", r#""invalid record""#, "null", "[]", "true"] {
        std::fs::write(&path, format!("{valid}\n{invalid}\n")).expect("write");
        let error = replay(config.path(), cwd, SESSION).expect_err("nonobject history record");
        assert!(error.to_string().contains("line 2"), "{error}");
    }
}

#[test]
fn tool_calls_and_results_replay_with_their_identity_and_payload() {
    let call = serde_json::json!({"type":"assistant","message":{"content":[
        {"type":"tool_use","id":"call-1","name":"Bash","input":{"command":"pwd"}}
    ]}});
    let result = serde_json::json!({"type":"user","message":{"content":[
        {"type":"tool_result","tool_use_id":"call-1","content":"/fixture/project","is_error":false}
    ]}});
    let call = line_updates(&call).expect("call replay");
    let result = line_updates(&result).expect("result replay");
    assert!(
        matches!(&call[0], SessionUpdate::ToolCall(call) if call.tool_call_id.to_string() == "call-1")
    );
    assert!(
        matches!(&result[0], SessionUpdate::ToolCallUpdate(update) if update.tool_call_id.to_string() == "call-1" && update.fields.raw_output == Some(serde_json::json!("/fixture/project")))
    );
}

#[test]
fn the_default_config_dir_is_under_home_unless_overridden() {
    let home = std::ffi::OsStr::new("/fixture/home");
    if !in_env::in_env(&[("HOME", Some(home)), ("CLAUDE_CONFIG_DIR", None)]) {
        return;
    }
    assert_eq!(default_config_dir(), Path::new(home).join(".claude"));
}

#[test]
fn an_explicit_config_directory_overrides_home() {
    let config = std::ffi::OsStr::new("/fixture/claude");
    if !in_env::in_env(&[("CLAUDE_CONFIG_DIR", Some(config))]) {
        return;
    }
    assert_eq!(default_config_dir(), Path::new(config));
}
