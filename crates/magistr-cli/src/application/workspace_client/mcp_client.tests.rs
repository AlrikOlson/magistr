use std::io::Write as _;
use std::path::PathBuf;

use super::*;

/// A whole `tools/call` result carrying `envelope` as structured content.
fn result(envelope: Value) -> Value {
    json!({ "structuredContent": envelope })
}

#[test]
fn tool_errors_preserve_diagnostics_before_decoding_success() {
    let original = json!({"isError":true, "content":[{"type":"text","text":"write outcome uncertain: inspect journal"}]});
    let error = envelope(&original).unwrap_err();
    assert!(
        error.contains("write outcome uncertain: inspect journal"),
        "{error}"
    );
    let original = json!({"isError":true, "structuredContent":{"ok":true,"results":["misleading"]},
        "content":[{"type":"text","text":"original failure"}]});
    assert!(
        envelope(&original)
            .unwrap_err()
            .contains("original failure")
    );
}

#[test]
fn a_false_envelope_keeps_recovery_and_effect_fields() {
    let original = result(
        json!({"v":1,"ok":false,"error":{"code":"ENGINE_BUSY","message":"busy",
        "recovery":"retry","backoffMs":50},"effect":"none"}),
    );
    let error = envelope(&original).unwrap_err();
    for evidence in ["ENGINE_BUSY", "backoffMs", "effect"] {
        assert!(error.contains(evidence), "{error}");
    }
}

#[test]
fn an_ok_envelope_comes_back_whole() {
    let answered = envelope(&result(json!({ "v": 1, "ok": true, "results": [1, 2] })))
        .expect("an ok envelope is not a refusal");
    assert_eq!(answered["results"], json!([1, 2]));
}

#[test]
fn a_refusal_becomes_the_workspaces_own_words() {
    let refused = envelope(&result(json!({
        "v": 1,
        "ok": false,
        "error": { "code": "COMMAND_NOT_DECLARED", "message": "command \"test\" is not declared" },
    })));
    let message = refused.unwrap_err();
    assert!(message.contains("COMMAND_NOT_DECLARED"));
    assert!(message.contains("is not declared"));
}

#[test]
fn a_refusal_without_a_message_still_reads_as_a_refusal() {
    let refused = envelope(&result(json!({ "v": 1, "ok": false })));
    assert!(refused.unwrap_err().contains("the workspace refused"));
}

#[test]
fn a_text_block_is_parsed_when_there_is_no_structured_content() {
    let answered = envelope(&json!({
        "content": [{ "type": "text", "text": "{\"v\":1,\"ok\":true,\"results\":[]}" }],
    }))
    .expect("the text block carries the same envelope");
    assert_eq!(answered["ok"], json!(true));
}

#[test]
fn an_answer_the_cockpit_cannot_read_is_refused_in_words() {
    let unreadable = envelope(&json!({ "content": [{ "type": "image" }] }));
    assert!(
        unreadable
            .unwrap_err()
            .contains("the workspace answered nothing the cockpit could read")
    );
}

/// Write a POSIX-sh MCP server that answers the handshake and one
/// `tools/call`, and hand back its path. Enough to prove the spawn,
/// the handshake order and the id matching against a real child.
fn fake_server(dir: &std::path::Path, envelope: &str) -> PathBuf {
    let path = dir.join("server.sh");
    let script = format!(
        r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo '{{"jsonrpc":"2.0","id":1,"result":{{"protocolVersion":"2025-06-18","capabilities":{{}}}}}}'
      ;;
    *'"method":"tools/call"'*)
      echo '{{"jsonrpc":"2.0","method":"notifications/message","params":{{}}}}'
      echo '{{"jsonrpc":"2.0","id":2,"result":{{"structuredContent":{envelope}}}}}'
      ;;
  esac
done
"#
    );
    let mut file = std::fs::File::create(&path).expect("the fake server is writable");
    file.write_all(script.as_bytes())
        .expect("the fake server is written");
    drop(file);
    path
}

/// A script path as `sh` wants it — with `/`, which MSYS's `sh` reads
/// and Unix's never sees.
fn sh_arg(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn a_spawned_server_is_handshaken_and_called() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let server = fake_server(
        dir.path(),
        r#"{"ok":true,"receipts":[],"value":{"v":1,"ok":true,"results":[{"kind":"file"}]}}"#,
    );
    let mut client = McpClient::connect("sh", &[sh_arg(&server)], &[], None)
        .expect("the fake server completes the handshake");
    let envelope = client
        .call("map", json!({}), Duration::from_secs(10))
        .expect("the fake server answers the call");
    // The notification sent ahead of the answer was skipped, not mistaken
    // for it.
    assert_eq!(envelope["results"][0]["kind"], json!("file"));
}

#[test]
fn a_server_that_refuses_to_start_says_why() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("locked.sh");
    std::fs::write(
        &path,
        "#!/bin/sh\necho 'another magistr instance (pid 3079) already owns this workspace' >&2\nexit 1\n",
    )
    .expect("the fake server is written");
    let refused = McpClient::connect("sh", &[sh_arg(&path)], &[], None)
        .err()
        .expect("a server that exits cannot be connected to");
    assert!(
        refused.contains("already owns this workspace"),
        "the server's last words are the reason, got {refused:?}"
    );
}

#[test]
fn a_command_that_does_not_exist_is_refused_in_words() {
    let refused = McpClient::connect("magistr-no-such-workspace-server", &[], &[], None)
        .err()
        .expect("a command that does not exist cannot be spawned");
    assert!(
        refused.starts_with("magistr-no-such-workspace-server did not start"),
        "got {refused:?}"
    );
}
